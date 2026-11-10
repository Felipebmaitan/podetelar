//! Explicit experimental I420 presentation. Excluded from default builds.
use eframe::{egui, egui_glow, glow};
use glow::HasContext as _;
use livekit::webrtc::video_frame::{BoxVideoBuffer, VideoBuffer};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

pub static UPDATES: AtomicU64 = AtomicU64::new(0);
pub static BYTES: AtomicU64 = AtomicU64::new(0);
pub static DRAWS: AtomicU64 = AtomicU64::new(0);
pub static FAULTS: AtomicU64 = AtomicU64::new(0);
pub static UPLOAD_NS: AtomicU64 = AtomicU64::new(0);
pub static MAX_AGE_US: AtomicU64 = AtomicU64::new(0);
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("FREEDOMSTREAM_DEEP_PLANAR").is_ok_and(|v| v == "1"))
}
pub struct Picture {
    pub buffer: BoxVideoBuffer,
    pub arrived: Instant,
}
impl Picture {
    pub fn prepare(buffer: BoxVideoBuffer) -> Result<Arc<Self>, BoxVideoBuffer> {
        let arrived = Instant::now();
        if let Some(planes) = buffer.as_i420() {
            // Force deferred readback on the receiver task, before the GUI callback.
            let (y, u, v) = planes.data();
            if !y.is_empty() && !u.is_empty() && !v.is_empty() {
                return Ok(Arc::new(Self { buffer, arrived }));
            }
        }
        Err(buffer)
    }
}
#[derive(Default)]
pub struct Videos {
    states: HashMap<String, Arc<Mutex<State>>>,
}
impl Videos {
    pub fn update(&mut self, id: &str, picture: Arc<Picture>) {
        self.states
            .entry(id.into())
            .or_default()
            .lock()
            .unwrap()
            .picture = Some(picture);
    }
    pub fn paint(&self, ui: &egui::Ui, id: &str, rect: egui::Rect) -> bool {
        let Some(state) = self.states.get(id) else {
            return false;
        };
        let aspect = {
            let state = state.lock().unwrap();
            let Some(p) = &state.picture else {
                return false;
            };
            p.buffer.width() as f32 / p.buffer.height() as f32
        };
        let size = egui::vec2(
            rect.width().min(rect.height() * aspect),
            rect.height().min(rect.width() / aspect),
        );
        let target = egui::Rect::from_center_size(rect.center(), size);
        let state = state.clone();
        ui.painter().add(egui::PaintCallback {
            rect: target,
            callback: Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                let mut state = state.lock().unwrap();
                if state.destroyed {
                    return;
                }
                // SAFETY: Glow invokes this callback with its owning GL context current.
                if unsafe { state.draw(painter.gl()) }.is_err() {
                    FAULTS.fetch_add(1, Ordering::Relaxed);
                }
            })),
        });
        true
    }
    pub fn retain(&mut self, gl: Option<&Arc<glow::Context>>, keep: impl Fn(&str) -> bool) {
        if let Some(gl) = gl {
            self.states.retain(|id, state| {
                if keep(id) {
                    true
                } else {
                    unsafe { state.lock().unwrap().destroy(gl) };
                    false
                }
            });
        }
    }
    pub fn destroy(&mut self, gl: Option<&glow::Context>) {
        if let Some(gl) = gl {
            for state in self.states.values() {
                unsafe { state.lock().unwrap().destroy(gl) }
            }
        }
        self.states.clear();
    }
}
#[derive(Default)]
pub struct State {
    picture: Option<Arc<Picture>>,
    uploaded: Option<Arc<Picture>>,
    program: Option<glow::Program>,
    vao: Option<glow::VertexArray>,
    textures: Vec<glow::Texture>,
    destroyed: bool,
}
const VERTEX: &str = r#"#version 330 core
out vec2 uv;
void main(){vec2 p=vec2((gl_VertexID==1)?3.0:-1.0,(gl_VertexID==2)?3.0:-1.0);gl_Position=vec4(p,0,1);uv=vec2((p.x+1.0)*0.5,(1.0-p.y)*0.5);}
"#;
const FRAGMENT: &str = r#"#version 330 core
in vec2 uv;out vec4 color;
uniform sampler2D plane_y;uniform sampler2D plane_u;uniform sampler2D plane_v;
vec3 rgb(ivec2 p){
 p=clamp(p,ivec2(0),textureSize(plane_y,0)-ivec2(1));
 int y=int(round(texelFetch(plane_y,p,0).r*255.0));
 int u=int(round(texelFetch(plane_u,p/2,0).r*255.0))-128;
 int v=int(round(texelFetch(plane_v,p/2,0).r*255.0))-128;
 int l=((y*257*18997)>>16)-1160;
 ivec3 q=ivec3(l+102*v,l-25*u-52*v,l+128*u)>>6;
 return vec3(clamp(q,ivec3(0),ivec3(255)))/255.0;
}
void main(){vec2 p=uv*vec2(textureSize(plane_y,0))-0.5;ivec2 b=ivec2(floor(p));vec2 f=fract(p);
 color=vec4(mix(mix(rgb(b),rgb(b+ivec2(1,0)),f.x),mix(rgb(b+ivec2(0,1)),rgb(b+ivec2(1,1)),f.x),f.y),1.0);}
"#;
impl State {
    unsafe fn initialize(&mut self, gl: &glow::Context) -> Result<(), String> {
        unsafe {
            let program = gl.create_program()?;
            for (kind, source) in [
                (glow::VERTEX_SHADER, VERTEX),
                (glow::FRAGMENT_SHADER, FRAGMENT),
            ] {
                let shader = gl.create_shader(kind)?;
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    let e = gl.get_shader_info_log(shader);
                    gl.delete_shader(shader);
                    gl.delete_program(program);
                    return Err(e);
                }
                gl.attach_shader(program, shader);
                gl.delete_shader(shader);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                let e = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(e);
            }
            self.program = Some(program);
            self.vao = Some(gl.create_vertex_array()?);
            for _ in 0..3 {
                self.textures.push(gl.create_texture()?);
            }
            Ok(())
        }
    }
    unsafe fn draw(&mut self, gl: &glow::Context) -> Result<(), String> {
        unsafe {
            if self.program.is_none() {
                self.initialize(gl)?;
            }
            let Some(picture) = self.picture.as_ref() else {
                return Ok(());
            };
            let planes = picture.buffer.as_i420().ok_or("Missing I420 owner")?;
            gl.use_program(self.program);
            gl.bind_vertex_array(self.vao);
            let old_alignment = gl.get_parameter_i32(glow::UNPACK_ALIGNMENT);
            let old_row = gl.get_parameter_i32(glow::UNPACK_ROW_LENGTH);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            let changed = self
                .uploaded
                .as_ref()
                .is_none_or(|old| !Arc::ptr_eq(old, picture));
            let start = Instant::now();
            let (y, u, v) = planes.data();
            let widths = [planes.width(), planes.chroma_width(), planes.chroma_width()];
            let heights = [
                planes.height(),
                planes.chroma_height(),
                planes.chroma_height(),
            ];
            let (sy, su, sv) = planes.strides();
            let strides = [sy, su, sv];
            for (index, data) in [y, u, v].iter().enumerate() {
                gl.active_texture(glow::TEXTURE0 + index as u32);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.textures[index]));
                if changed {
                    for n in [glow::TEXTURE_MIN_FILTER, glow::TEXTURE_MAG_FILTER] {
                        gl.tex_parameter_i32(glow::TEXTURE_2D, n, glow::NEAREST as i32)
                    }
                    for n in [glow::TEXTURE_WRAP_S, glow::TEXTURE_WRAP_T] {
                        gl.tex_parameter_i32(glow::TEXTURE_2D, n, glow::CLAMP_TO_EDGE as i32)
                    }
                    gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, strides[index] as i32);
                    gl.tex_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        glow::R8 as i32,
                        widths[index] as i32,
                        heights[index] as i32,
                        0,
                        glow::RED,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(Some(data)),
                    );
                    BYTES.fetch_add(
                        widths[index] as u64 * heights[index] as u64,
                        Ordering::Relaxed,
                    );
                }
                let name = ["plane_y", "plane_u", "plane_v"][index];
                gl.uniform_1_i32(
                    gl.get_uniform_location(self.program.unwrap(), name)
                        .as_ref(),
                    index as i32,
                );
            }
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, old_alignment);
            gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, old_row);
            if changed {
                self.uploaded = Some(picture.clone());
                UPDATES.fetch_add(1, Ordering::Relaxed);
                UPLOAD_NS.fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
            MAX_AGE_US.fetch_max(
                picture.arrived.elapsed().as_micros() as u64,
                Ordering::Relaxed,
            );
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            DRAWS.fetch_add(1, Ordering::Relaxed);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_vertex_array(None);
            gl.use_program(None);
            if gl.get_error() != glow::NO_ERROR {
                return Err("Planar GL error".into());
            }
            Ok(())
        }
    }
    unsafe fn destroy(&mut self, gl: &glow::Context) {
        unsafe {
            self.destroyed = true;
            for texture in self.textures.drain(..) {
                gl.delete_texture(texture)
            }
            if let Some(program) = self.program.take() {
                gl.delete_program(program)
            }
            if let Some(vao) = self.vao.take() {
                gl.delete_vertex_array(vao)
            }
            self.picture = None;
            self.uploaded = None;
        }
    }
}

/// Pixel qualification in the actual GL context. No media/network/screen capture.
pub fn check(report: std::path::PathBuf) -> anyhow::Result<()> {
    struct Check {
        report: std::path::PathBuf,
        done: bool,
    }
    impl eframe::App for Check {
        fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
            if self.done {
                return;
            }
            self.done = true;
            let result = frame
                .gl()
                .ok_or_else(|| "No GL context".to_owned())
                .and_then(|gl| unsafe { check_pixels(gl) });
            let value = match result {
                Ok(rows) => {
                    serde_json::json!({"completed":rows.iter().all(|r|r["passed"]==true),"maxAllowedComponentError":2,"maxAllowedMeanError":0.35,"cases":rows})
                }
                Err(e) => serde_json::json!({"completed":false,"fault":e}),
            };
            let _ = std::fs::write(&self.report, serde_json::to_vec_pretty(&value).unwrap());
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default().with_inner_size([480.0, 270.0]),
        ..Default::default()
    };
    eframe::run_native(
        "FreedomStream planar pixel qualification",
        options,
        Box::new(move |_| {
            Ok(Box::new(Check {
                report,
                done: false,
            }))
        }),
    )
    .map_err(|_| anyhow::anyhow!("Planar qualification could not run"))
}
unsafe fn check_pixels(gl: &glow::Context) -> Result<Vec<serde_json::Value>, String> {
    use livekit::webrtc::video_frame::{I420Buffer, VideoFormatType};
    unsafe {
        let old_fb = gl.get_parameter_framebuffer(glow::FRAMEBUFFER_BINDING);
        let mut old_view = [0; 4];
        gl.get_parameter_i32_slice(glow::VIEWPORT, &mut old_view);
        gl.disable(glow::FRAMEBUFFER_SRGB);
        gl.disable(glow::BLEND);
        gl.disable(glow::SCISSOR_TEST);
        let reference = gl.create_program()?;
        for (kind, source) in [
            (glow::VERTEX_SHADER, VERTEX),
            (
                glow::FRAGMENT_SHADER,
                "#version 330 core\nin vec2 uv;out vec4 color;uniform sampler2D image;void main(){color=texture(image,uv);}",
            ),
        ] {
            let shader = gl.create_shader(kind)?;
            gl.shader_source(shader, source);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                return Err(gl.get_shader_info_log(shader));
            }
            gl.attach_shader(reference, shader);
            gl.delete_shader(shader);
        }
        gl.link_program(reference);
        if !gl.get_program_link_status(reference) {
            return Err(gl.get_program_info_log(reference));
        }
        let fbo = gl.create_framebuffer()?;
        let output = gl.create_texture()?;
        let rgba_tex = gl.create_texture()?;
        let mut rows = Vec::new();
        for (width, height) in [(17u32, 9u32), (321, 181), (1920, 1080)] {
            for pattern in 0..3 {
                let mut buffer = I420Buffer::with_strides(
                    width,
                    height,
                    width + 7,
                    width.div_ceil(2) + 5,
                    width.div_ceil(2) + 3,
                );
                let (sy, su, sv) = buffer.strides();
                let (y, u, v) = buffer.data_mut();
                for (plane, stride, pw, ph, index) in [
                    (y, sy, width, height, 0),
                    (u, su, width.div_ceil(2), height.div_ceil(2), 1),
                    (v, sv, width.div_ceil(2), height.div_ceil(2), 2),
                ] {
                    plane.fill(173);
                    for yy in 0..ph {
                        for xx in 0..pw {
                            plane[(yy * stride + xx) as usize] = match pattern {
                                0 => {
                                    if index == 0 {
                                        if xx % 7 == 0 || yy % 11 == 0 { 235 } else { 16 }
                                    } else {
                                        128
                                    }
                                }
                                1 => ((xx * 251 / pw + yy * 17 + index * 61) % 256) as u8,
                                _ => ((xx * 37 + yy * 93 + index * 79) % 256) as u8,
                            };
                        }
                    }
                }
                let mut rgba = vec![0; (width * height * 4) as usize];
                buffer.as_ref().to_argb(
                    VideoFormatType::ABGR,
                    &mut rgba,
                    width * 4,
                    width as i32,
                    height as i32,
                );
                let picture =
                    Picture::prepare(Box::new(buffer)).map_err(|_| "I420 preparation failed")?;
                let mut state = State {
                    picture: Some(picture),
                    ..Default::default()
                };
                for (ow, oh) in [(width, height), (303, 171), (960, 540)] {
                    gl.bind_texture(glow::TEXTURE_2D, Some(output));
                    gl.tex_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        glow::RGBA8 as i32,
                        ow as i32,
                        oh as i32,
                        0,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(None),
                    );
                    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
                    gl.framebuffer_texture_2d(
                        glow::FRAMEBUFFER,
                        glow::COLOR_ATTACHMENT0,
                        glow::TEXTURE_2D,
                        Some(output),
                        0,
                    );
                    if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE
                    {
                        return Err("Incomplete validation framebuffer".into());
                    }
                    gl.viewport(0, 0, ow as i32, oh as i32);
                    state.draw(gl)?;
                    let mut actual = vec![0u8; (ow * oh * 4) as usize];
                    gl.read_pixels(
                        0,
                        0,
                        ow as i32,
                        oh as i32,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelPackData::Slice(Some(&mut actual)),
                    );
                    gl.active_texture(glow::TEXTURE0);
                    gl.bind_texture(glow::TEXTURE_2D, Some(rgba_tex));
                    gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
                    gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
                    for n in [glow::TEXTURE_MIN_FILTER, glow::TEXTURE_MAG_FILTER] {
                        gl.tex_parameter_i32(glow::TEXTURE_2D, n, glow::LINEAR as i32)
                    }
                    for n in [glow::TEXTURE_WRAP_S, glow::TEXTURE_WRAP_T] {
                        gl.tex_parameter_i32(glow::TEXTURE_2D, n, glow::CLAMP_TO_EDGE as i32)
                    }
                    gl.tex_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        glow::RGBA8 as i32,
                        width as i32,
                        height as i32,
                        0,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(Some(&rgba)),
                    );
                    gl.use_program(Some(reference));
                    gl.bind_vertex_array(state.vao);
                    gl.uniform_1_i32(gl.get_uniform_location(reference, "image").as_ref(), 0);
                    gl.draw_arrays(glow::TRIANGLES, 0, 3);
                    let mut expected = vec![0u8; actual.len()];
                    gl.read_pixels(
                        0,
                        0,
                        ow as i32,
                        oh as i32,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelPackData::Slice(Some(&mut expected)),
                    );
                    let mut max = 0u8;
                    let mut total = 0u64;
                    let mut changed = 0usize;
                    for (a, b) in actual.iter().zip(&expected) {
                        let d = a.abs_diff(*b);
                        max = max.max(d);
                        total += d as u64;
                        changed += usize::from(d != 0);
                    }
                    let mean = total as f64 / actual.len() as f64;
                    rows.push(serde_json::json!({"source":[width,height],"output":[ow,oh],"pattern":pattern,"components":actual.len(),"maxError":max,"meanAbsoluteError":mean,"differentComponents":changed,"passed":max<=2&&mean<=0.35}));
                }
                state.destroy(gl);
            }
        }
        gl.bind_framebuffer(glow::FRAMEBUFFER, old_fb);
        gl.viewport(old_view[0], old_view[1], old_view[2], old_view[3]);
        gl.bind_vertex_array(None);
        gl.use_program(None);
        gl.delete_framebuffer(fbo);
        gl.delete_texture(output);
        gl.delete_texture(rgba_tex);
        gl.delete_program(reference);
        if gl.get_error() != glow::NO_ERROR {
            return Err("Validation GL error".into());
        }
        Ok(rows)
    }
}
