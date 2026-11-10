//! Explicit experimental I420 presentation. Excluded from default builds.
use eframe::{egui, egui_glow, glow};
use glow::HasContext as _;
use livekit::webrtc::video_frame::{BoxVideoBuffer, VideoBuffer};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

pub static UPDATES: AtomicU64 = AtomicU64::new(0);
pub static BYTES: AtomicU64 = AtomicU64::new(0);
pub static DRAWS: AtomicU64 = AtomicU64::new(0);
pub static FAULTS: AtomicU64 = AtomicU64::new(0);
pub static UPLOAD_NS: AtomicU64 = AtomicU64::new(0);
pub static MAX_AGE_US: AtomicU64 = AtomicU64::new(0);
static FAILED: AtomicBool = AtomicBool::new(false);
pub fn retain_planes() -> bool {
    static RETAIN: OnceLock<bool> = OnceLock::new();
    *RETAIN
        .get_or_init(|| std::env::var("FREEDOMSTREAM_DEEP_RETAIN_PLANES").is_ok_and(|v| v == "1"))
}
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("FREEDOMSTREAM_DEEP_PLANAR").is_ok_and(|v| v == "1"))
        && !FAILED.load(Ordering::Relaxed)
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
    pub fn rgba(&mut self, id: &str) {
        if let Some(state) = self.states.get(id) {
            let mut state = state.lock().unwrap();
            state.pending = None;
            state.uploaded = None;
            state.retained = None;
        }
    }
    pub fn has_picture(&self, id: &str) -> bool {
        self.states
            .get(id)
            .is_some_and(|state| state.lock().unwrap().dimensions().is_some())
    }
    /// Qualification-only drawing; the caller must have this GL context current.
    pub unsafe fn qualify_draw(&self, id: &str, gl: &glow::Context) -> Result<(), String> {
        let state = self.states.get(id).ok_or("Missing qualification picture")?;
        unsafe { state.lock().unwrap().draw(gl) }
    }
    pub fn update(&mut self, id: &str, picture: Arc<Picture>) {
        self.states
            .entry(id.into())
            .or_default()
            .lock()
            .unwrap()
            .pending = Some(picture);
    }
    pub fn paint(&self, ui: &egui::Ui, id: &str, rect: egui::Rect) -> bool {
        if FAILED.load(Ordering::Relaxed) {
            return false;
        }
        let Some(state) = self.states.get(id) else {
            return false;
        };
        let aspect = {
            let state = state.lock().unwrap();
            let Some([width, height]) = state.dimensions() else {
                return false;
            };
            width as f32 / height as f32
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
                if state.destroyed || FAILED.load(Ordering::Relaxed) {
                    return;
                }
                // SAFETY: Glow invokes this callback with its owning GL context current.
                let result = unsafe { state.draw(painter.gl()) };
                unsafe {
                    state.finish_draw(painter.gl(), result);
                }
            })),
        });
        true
    }
    pub fn retain(&mut self, gl: Option<&Arc<glow::Context>>, keep: impl Fn(&str) -> bool) {
        if let Some(gl) = gl {
            self.states.retain(|id, state| {
                let mut state = state.lock().unwrap();
                if keep(id) && state.dimensions().is_some() {
                    true
                } else {
                    unsafe { state.destroy(gl) };
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
    pending: Option<Arc<Picture>>,
    uploaded: Option<Uploaded>,
    retained: Option<Arc<Picture>>,
    program: Option<glow::Program>,
    vao: Option<glow::VertexArray>,
    textures: Vec<glow::Texture>,
    destroyed: bool,
}
struct Uploaded {
    size: [u32; 2],
    arrived: Instant,
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
    fn dimensions(&self) -> Option<[u32; 2]> {
        self.pending
            .as_ref()
            .map(|p| [p.buffer.width(), p.buffer.height()])
            .or_else(|| self.uploaded.as_ref().map(|u| u.size))
    }
    unsafe fn finish_draw(&mut self, gl: &glow::Context, result: Result<(), String>) -> bool {
        if result.is_ok() {
            return true;
        }
        FAULTS.fetch_add(1, Ordering::Relaxed);
        // The receiver observes this before preparing another picture and uses
        // its regular RGBA path. No repeated shader failures or partial state.
        FAILED.store(true, Ordering::Relaxed);
        unsafe {
            self.destroy(gl);
        }
        false
    }
    unsafe fn initialize(&mut self, gl: &glow::Context) -> Result<(), String> {
        unsafe { self.initialize_at(gl, None) }
    }
    // Fault injection follows the same cleanup path as create_* failures.
    unsafe fn initialize_at(
        &mut self,
        gl: &glow::Context,
        fail_at: Option<usize>,
    ) -> Result<(), String> {
        unsafe {
            let mut pending = State::default();
            let mut step = 0;
            let mut checkpoint = || {
                step += 1;
                if fail_at == Some(step) {
                    Err("Injected GL allocation failure".to_owned())
                } else {
                    Ok(())
                }
            };
            let built = (|| -> Result<(), String> {
                checkpoint()?;
                let program = gl.create_program()?;
                pending.program = Some(program);
                for (kind, source) in [
                    (glow::VERTEX_SHADER, VERTEX),
                    (glow::FRAGMENT_SHADER, FRAGMENT),
                ] {
                    checkpoint()?;
                    let shader = gl.create_shader(kind)?;
                    gl.shader_source(shader, source);
                    gl.compile_shader(shader);
                    if !gl.get_shader_compile_status(shader) {
                        let e = gl.get_shader_info_log(shader);
                        gl.delete_shader(shader);
                        return Err(e);
                    }
                    gl.attach_shader(program, shader);
                    gl.delete_shader(shader);
                }
                gl.link_program(program);
                if !gl.get_program_link_status(program) {
                    let e = gl.get_program_info_log(program);
                    return Err(e);
                }
                checkpoint()?;
                pending.vao = Some(gl.create_vertex_array()?);
                for _ in 0..3 {
                    checkpoint()?;
                    pending.textures.push(gl.create_texture()?);
                }
                checkpoint()?;
                Ok(())
            })();
            if let Err(error) = built {
                pending.destroy(gl);
                return Err(error);
            }
            self.program = pending.program.take();
            self.vao = pending.vao.take();
            self.textures = std::mem::take(&mut pending.textures);
            Ok(())
        }
    }
    unsafe fn draw(&mut self, gl: &glow::Context) -> Result<(), String> {
        unsafe {
            if self.program.is_none() {
                self.initialize(gl)?;
            }
            if self.dimensions().is_none() {
                return Ok(());
            }
            let picture = self.pending.take();
            let planes = picture
                .as_ref()
                .map(|p| p.buffer.as_i420().ok_or("Missing I420 owner"))
                .transpose()?;
            let data = planes.map(|p| {
                let (y, u, v) = p.data();
                [y, u, v]
            });
            let widths = planes
                .map(|p| [p.width(), p.chroma_width(), p.chroma_width()])
                .unwrap_or([0; 3]);
            let heights = planes
                .map(|p| [p.height(), p.chroma_height(), p.chroma_height()])
                .unwrap_or([0; 3]);
            let strides = planes
                .map(|p| {
                    let (y, u, v) = p.strides();
                    [y, u, v]
                })
                .unwrap_or([0; 3]);
            gl.use_program(self.program);
            gl.bind_vertex_array(self.vao);
            let unpack = picture.as_ref().map(|_| {
                (
                    gl.get_parameter_i32(glow::UNPACK_ALIGNMENT),
                    gl.get_parameter_i32(glow::UNPACK_ROW_LENGTH),
                    gl.get_parameter_buffer(glow::PIXEL_UNPACK_BUFFER_BINDING),
                )
            });
            if unpack.is_some() {
                gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
                gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
            }
            let start = Instant::now();
            for index in 0..3 {
                gl.active_texture(glow::TEXTURE0 + index as u32);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.textures[index]));
                if let Some(data) = data.as_ref() {
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
                        glow::PixelUnpackData::Slice(Some(data[index])),
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
            if let Some((alignment, row, buffer)) = unpack {
                gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, alignment);
                gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, row);
                gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, buffer);
            }
            if let Some(picture) = picture {
                self.uploaded = Some(Uploaded {
                    size: [picture.buffer.width(), picture.buffer.height()],
                    arrived: picture.arrived,
                });
                // Client-memory texImage2D has consumed all planes before returning.
                // Keep only GPU pixels and scalar metadata for subsequent redraws.
                self.retained = retain_planes().then_some(picture);
                UPDATES.fetch_add(1, Ordering::Relaxed);
                UPLOAD_NS.fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
            MAX_AGE_US.fetch_max(
                self.uploaded
                    .as_ref()
                    .unwrap()
                    .arrived
                    .elapsed()
                    .as_micros() as u64,
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
            self.pending = None;
            self.uploaded = None;
            self.retained = None;
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
                .and_then(|gl| unsafe {
                    let mut lifecycle = check_initialization(gl)?;
                    lifecycle.extend(
                        crate::ui::experiment::qualify_planar_transitions(ui, gl).map_err(
                            |_| "Representation transition qualification failed".to_owned(),
                        )?,
                    );
                    let pixels = check_pixels(gl)?;
                    lifecycle.push(check_failover(gl)?);
                    Ok((pixels, lifecycle))
                });
            let value = match result {
                Ok((rows, lifecycle)) => {
                    serde_json::json!({"completed":rows.iter().all(|r|r["passed"]==true)&&lifecycle.iter().all(|r|r["passed"]==true),"maxAllowedComponentError":2,"maxAllowedMeanError":0.35,"cases":rows,"lifecycle":lifecycle})
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
unsafe fn check_initialization(gl: &glow::Context) -> Result<Vec<serde_json::Value>, String> {
    let mut rows = Vec::new();
    for fail_at in 1..=8 {
        let mut state = State::default();
        let failed = unsafe { state.initialize_at(gl, Some(fail_at)) }.is_err();
        let unchanged = state.program.is_none() && state.vao.is_none() && state.textures.is_empty();
        let retry = unsafe { state.initialize(gl) }.is_ok();
        let complete = state.program.is_some() && state.vao.is_some() && state.textures.len() == 3;
        unsafe {
            state.destroy(gl);
        }
        if !failed
            || !unchanged
            || !retry
            || !complete
            || unsafe { gl.get_error() } != glow::NO_ERROR
        {
            return Err(format!(
                "GL initialization cleanup/retry failed at stage {fail_at}"
            ));
        }
        rows.push(serde_json::json!({"test":"GL allocation failure and retry","stage":fail_at,"passed":true}));
    }
    Ok(rows)
}
unsafe fn check_failover(gl: &glow::Context) -> Result<serde_json::Value, String> {
    use livekit::webrtc::video_frame::I420Buffer;
    let mut buffer = I420Buffer::new(17, 9);
    let (y, u, v) = buffer.data_mut();
    y.fill(16);
    u.fill(128);
    v.fill(128);
    let owner =
        Picture::prepare(Box::new(buffer)).map_err(|_| "Cannot prepare failover picture")?;
    let mut state = State {
        pending: Some(owner.clone()),
        ..Default::default()
    };
    unsafe {
        state.draw(gl)?;
    }
    let program = state.program.unwrap();
    let vao = state.vao.unwrap();
    let textures = state.textures.clone();
    state.pending = Some(owner.clone());
    let faults = FAULTS.load(Ordering::Relaxed);
    let initially_available = !FAILED.load(Ordering::Relaxed);
    unsafe {
        gl.bind_vertex_array(Some(vao));
    }
    let rejected = !unsafe { state.finish_draw(gl, Err("Injected renderer fault".into())) };
    let deleted = unsafe {
        !gl.is_program(program)
            && gl.get_parameter_i32(glow::VERTEX_ARRAY_BINDING) == 0
            && textures.iter().all(|t| !gl.is_texture(*t))
    };
    if !initially_available
        || !rejected
        || enabled()
        || !FAILED.load(Ordering::Relaxed)
        || FAULTS.load(Ordering::Relaxed) != faults + 1
        || !deleted
        || Arc::strong_count(&owner) != 1
    {
        return Err("Renderer failover or GL/frame cleanup failed".into());
    }
    Ok(
        serde_json::json!({"test":"GL failure selects RGBA and releases actual GL/frame owners","passed":true}),
    )
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
                    pending: Some(picture),
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
