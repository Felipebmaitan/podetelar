//! Synthetic GPU upload probe in the real eframe/Glow context. No media or network.
//! Results are JSON files because this executable deliberately has no console.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::{egui, glow};
use glow::HasContext as _;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

const WARMUP: usize = 30;

struct Config {
    sub: bool,
    width: usize,
    tiles: usize,
    frames: usize,
    seconds: u64,
    vsync: bool,
    finish: bool,
    report: PathBuf,
}

impl Config {
    fn parse() -> Result<Self, String> {
        let mut c = Self {
            sub: false,
            width: 1920,
            tiles: 1,
            frames: 240,
            seconds: 20,
            vsync: false,
            finish: false,
            report: ".local/experiments/gpu-common/result.json".into(),
        };
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.len() % 2 != 0 {
            return Err("Options require values.".into());
        }
        for pair in args.chunks_exact(2) {
            let value = &pair[1];
            match pair[0].as_str() {
                "--mode" if value == "full" || value == "sub" => c.sub = value == "sub",
                "--width" => c.width = value.parse().map_err(|_| "Invalid width.")?,
                "--tiles" => c.tiles = value.parse().map_err(|_| "Invalid tiles.")?,
                "--frames" => c.frames = value.parse().map_err(|_| "Invalid frames.")?,
                "--seconds" => c.seconds = value.parse().map_err(|_| "Invalid seconds.")?,
                "--vsync" if value == "0" || value == "1" => c.vsync = value == "1",
                "--sync" if value == "queue" || value == "finish" => c.finish = value == "finish",
                "--report" => c.report = value.into(),
                _ => return Err("Unknown option or invalid value.".into()),
            }
        }
        if ![1920, 3840].contains(&c.width)
            || !(1..=4).contains(&c.tiles)
            || !(30..=5000).contains(&c.frames)
            || !(3..=120).contains(&c.seconds)
        {
            return Err("Use width 1920/3840, tiles 1..4, frames 30..5000, seconds 3..120.".into());
        }
        Ok(c)
    }

    fn height(&self) -> usize {
        self.width * 9 / 16
    }
}

struct Texture {
    native: glow::Texture,
    id: egui::TextureId,
}

struct Probe {
    config: Config,
    gl: Arc<glow::Context>,
    capabilities: Value,
    pixels: [Vec<u8>; 2],
    textures: Vec<Texture>,
    started: Instant,
    measured_start: Option<Instant>,
    measured_cpu: Option<u64>,
    last_frame: Option<Instant>,
    upload_ms: Vec<f64>,
    frame_ms: Vec<f64>,
    total_frames: usize,
    full_calls: usize,
    sub_calls: usize,
    measured_full_calls: usize,
    measured_sub_calls: usize,
    last_pattern: usize,
    reported: bool,
    fault: Option<String>,
}

impl Probe {
    fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Result<Self, String> {
        let gl = cc.gl.clone().ok_or("Glow context unavailable.")?;
        // One upload sample per rendered frame, even while Grid measures layout.
        cc.egui_ctx.options_mut(|options| {
            options.max_passes = std::num::NonZeroUsize::new(1).unwrap();
        });
        let capabilities = capabilities(cc, &gl);
        let pixels = [
            pattern(config.width, config.height(), 0),
            pattern(config.width, config.height(), 1),
        ];
        let ctx = cc.egui_ctx.clone();
        let deadline = config.seconds;
        // A second wakeup requests close even when no frame is being painted.
        // A hung GPU still requires the launcher's external process timeout.
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(deadline));
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            ctx.request_repaint();
        });
        Ok(Self {
            config,
            gl,
            capabilities,
            pixels,
            textures: Vec::new(),
            started: Instant::now(),
            measured_start: None,
            measured_cpu: None,
            last_frame: None,
            upload_ms: Vec::new(),
            frame_ms: Vec::new(),
            total_frames: 0,
            full_calls: 0,
            sub_calls: 0,
            measured_full_calls: 0,
            measured_sub_calls: 0,
            last_pattern: 0,
            reported: false,
            fault: None,
        })
    }

    fn initialize(&mut self, frame: &mut eframe::Frame) -> Result<(), String> {
        let gl = &self.gl;
        // App::ui has the current eframe GL context. Native texture ownership is
        // transferred to eframe; this probe must never delete registered names.
        unsafe {
            if gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE) < self.config.width as i32 {
                return Err("Requested texture exceeds MAX_TEXTURE_SIZE.".into());
            }
            let old = gl.get_parameter_texture(glow::TEXTURE_BINDING_2D);
            let alignment = gl.get_parameter_i32(glow::UNPACK_ALIGNMENT);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            for _ in 0..self.config.tiles {
                let native = gl.create_texture()?;
                gl.bind_texture(glow::TEXTURE_2D, Some(native));
                for name in [glow::TEXTURE_MIN_FILTER, glow::TEXTURE_MAG_FILTER] {
                    gl.tex_parameter_i32(glow::TEXTURE_2D, name, glow::LINEAR as i32);
                }
                for name in [glow::TEXTURE_WRAP_S, glow::TEXTURE_WRAP_T] {
                    gl.tex_parameter_i32(glow::TEXTURE_2D, name, glow::CLAMP_TO_EDGE as i32);
                }
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA8 as i32,
                    self.config.width as i32,
                    self.config.height() as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(&self.pixels[0])),
                );
                self.full_calls += 1;
                self.textures.push(Texture {
                    native,
                    id: frame.register_native_glow_texture(native),
                });
            }
            gl.bind_texture(glow::TEXTURE_2D, old);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, alignment);
            let error = gl.get_error();
            if error != glow::NO_ERROR {
                return Err(format!("GL initialization error 0x{error:x}."));
            }
        }
        Ok(())
    }

    fn upload(&mut self) -> Result<f64, String> {
        let gl = &self.gl;
        self.last_pattern = self.total_frames % 2;
        let pixels = &self.pixels[self.last_pattern];
        let before = Instant::now();
        unsafe {
            let old = gl.get_parameter_texture(glow::TEXTURE_BINDING_2D);
            let alignment = gl.get_parameter_i32(glow::UNPACK_ALIGNMENT);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            for texture in &self.textures {
                gl.bind_texture(glow::TEXTURE_2D, Some(texture.native));
                if self.config.sub {
                    gl.tex_sub_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        0,
                        0,
                        self.config.width as i32,
                        self.config.height() as i32,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(Some(pixels)),
                    );
                    self.sub_calls += 1;
                } else {
                    gl.tex_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        glow::RGBA8 as i32,
                        self.config.width as i32,
                        self.config.height() as i32,
                        0,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(Some(pixels)),
                    );
                    self.full_calls += 1;
                }
            }
            if self.config.finish {
                gl.finish();
            }
            gl.bind_texture(glow::TEXTURE_2D, old);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, alignment);
        }
        let elapsed = before.elapsed().as_secs_f64() * 1000.0;
        let error = unsafe { gl.get_error() };
        if error != glow::NO_ERROR {
            return Err(format!("GL upload error 0x{error:x}."));
        }
        Ok(elapsed)
    }

    fn verify_texels(&self) -> Result<usize, String> {
        let gl = &self.gl;
        let mut checked = 0;
        // Reads four texels from each synthetic texture, never the window/screen.
        unsafe {
            let previous = gl.get_parameter_framebuffer(glow::READ_FRAMEBUFFER_BINDING);
            let fbo = gl.create_framebuffer()?;
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(fbo));
            let mut valid = true;
            for texture in &self.textures {
                gl.framebuffer_texture_2d(
                    glow::READ_FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    glow::TEXTURE_2D,
                    Some(texture.native),
                    0,
                );
                if gl.check_framebuffer_status(glow::READ_FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE
                {
                    valid = false;
                    break;
                }
                for (x, y) in [
                    (0, 0),
                    (self.config.width - 1, 0),
                    (0, self.config.height() - 1),
                    (self.config.width - 1, self.config.height() - 1),
                ] {
                    let mut actual = [0_u8; 4];
                    gl.read_pixels(
                        x as i32,
                        y as i32,
                        1,
                        1,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelPackData::Slice(Some(&mut actual)),
                    );
                    let offset = (y * self.config.width + x) * 4;
                    valid &= actual == self.pixels[self.last_pattern][offset..offset + 4];
                    checked += 1;
                }
            }
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, previous);
            gl.delete_framebuffer(fbo);
            if gl.get_error() != glow::NO_ERROR || !valid {
                return Err("Synthetic texture texel verification failed.".into());
            }
        }
        Ok(checked)
    }

    fn report(&mut self, reason: &str) {
        if self.reported {
            return;
        }
        self.reported = true;
        // Drain the final queued upload and draw before the measurement ends.
        // Texel verification and JSON serialization remain outside the window.
        unsafe { self.gl.finish() };
        let wall = self.measured_start.map(|s| s.elapsed().as_secs_f64());
        let cpu = self
            .measured_cpu
            .zip(process_cpu_100ns())
            .map(|(a, b)| b.saturating_sub(a) as f64 / 10_000_000.0);
        let texels = if self.textures.is_empty() {
            Err("No textures initialized.".into())
        } else {
            self.verify_texels()
        };
        let frame_bytes = self.config.width * self.config.height() * 4 * self.config.tiles;
        let result = json!({
            "schema": 1, "experiment": "actual-eframe-glow-synthetic-upload", "reason": reason,
            "width": self.config.width, "height": self.config.height(), "tiles": self.config.tiles,
            "mode": if self.config.sub { "tex_sub_image_2d" } else { "tex_image_2d" },
            "sync": if self.config.finish { "finish" } else { "queue" }, "vsync": self.config.vsync,
            "warmupFrames": WARMUP, "totalFrames": self.total_frames, "measuredFrames": self.upload_ms.len(),
            "requestedFrames": self.config.frames, "completedRequestedFrames": self.upload_ms.len() == self.config.frames,
            "wallSeconds": wall, "processCpuSeconds": cpu,
            "processCpuOneCorePercent": cpu.zip(wall).map(|(c,w)| c / w * 100.0),
            "uploadCallCpuMs": distribution(&self.upload_ms), "frameIntervalMs": distribution(&self.frame_ms),
            "texImageCallsIncludingInitializationAndWarmup": self.full_calls,
            "texSubImageCallsIncludingWarmup": self.sub_calls,
            "measuredTexImageCalls": self.measured_full_calls, "measuredTexSubImageCalls": self.measured_sub_calls,
            "measuredSubmittedBytes": frame_bytes as u64 * self.upload_ms.len() as u64,
            "measuredSubmittedGiBPerSecond": wall.map(|w| frame_bytes as f64 * self.upload_ms.len() as f64 / w / 1_073_741_824.0),
            "verifiedSyntheticTexels": texels.as_ref().ok(), "verificationError": texels.err(), "fault": self.fault,
            "capabilities": self.capabilities,
            "limits": "CPU upload-call timing includes driver work and optional glFinish, not GPU elapsed time. Frame intervals include event-loop/swap pacing. No decode, conversion, subscription, screen capture, native-buffer import or end-to-end video claim."
        });
        let _ = std::fs::write(
            &self.config.report,
            serde_json::to_vec_pretty(&result).unwrap(),
        );
    }
}

impl eframe::App for Probe {
    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
        if self.reported {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.upload_ms.len() >= self.config.frames {
            self.report("frames-complete");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.textures.is_empty()
            && let Err(error) = self.initialize(frame)
        {
            self.fault = Some(error);
            self.report("initialization-failed");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.started.elapsed() >= Duration::from_secs(self.config.seconds) {
            self.report("deadline");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.total_frames == WARMUP {
            unsafe { self.gl.finish() };
            self.measured_start = Some(Instant::now());
            self.measured_cpu = process_cpu_100ns();
            self.last_frame = None;
        }
        let now = Instant::now();
        if self.total_frames >= WARMUP
            && let Some(last) = self.last_frame
        {
            self.frame_ms
                .push(now.duration_since(last).as_secs_f64() * 1000.0);
        }
        self.last_frame = Some(now);
        match self.upload() {
            Ok(ms) if self.total_frames >= WARMUP => {
                self.upload_ms.push(ms);
                if self.config.sub {
                    self.measured_sub_calls += self.config.tiles;
                } else {
                    self.measured_full_calls += self.config.tiles;
                }
            }
            Ok(_) => (),
            Err(error) => {
                self.fault = Some(error);
                self.report("upload-failed");
            }
        }
        self.total_frames += 1;
        egui::CentralPanel::default().show(root, |ui| {
            ui.label(format!(
                "Synthetic {} · {}×{} · {} tiles · {}/{} frames",
                if self.config.sub {
                    "reuse / sub image"
                } else {
                    "full image"
                },
                self.config.width,
                self.config.height(),
                self.config.tiles,
                self.upload_ms.len(),
                self.config.frames
            ));
            let columns = if self.config.tiles > 1 { 2 } else { 1 };
            let width = (ui.available_width() - 8.0) / columns as f32;
            egui::Grid::new("synthetic-textures").show(ui, |ui| {
                for (index, texture) in self.textures.iter().enumerate() {
                    ui.image((texture.id, egui::vec2(width, width * 9.0 / 16.0)));
                    if (index + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
        });
        // The next frame reports and closes after this image has been painted.
        ctx.request_repaint();
    }

    fn on_exit(&mut self, _gl: Option<&glow::Context>) {
        self.report("window-closed-or-watchdog");
    }
}

fn pattern(width: usize, height: usize, phase: usize) -> Vec<u8> {
    let mut pixels = vec![0; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let offset = (y * width + x) * 4;
            pixels[offset..offset + 4].copy_from_slice(&[
                ((x * 13 + y * 3 + phase * 71) % 256) as u8,
                ((x * 7 + y * 11 + phase * 41) % 256) as u8,
                ((x * 5 + y * 17 + phase * 23) % 256) as u8,
                255,
            ]);
        }
    }
    pixels
}

fn distribution(samples: &[f64]) -> Value {
    if samples.is_empty() {
        return Value::Null;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let at = |p: usize| sorted[(sorted.len() - 1) * p / 100];
    json!({"count": sorted.len(), "p50": at(50), "p95": at(95), "p99": at(99), "max": at(100)})
}

#[cfg(windows)]
fn process_cpu_100ns() -> Option<u64> {
    use windows::Win32::{
        Foundation::FILETIME,
        System::Threading::{GetCurrentProcess, GetProcessTimes},
    };
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
        .ok()?;
    }
    let value = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    Some(value(kernel) + value(user))
}

#[cfg(not(windows))]
fn process_cpu_100ns() -> Option<u64> {
    None
}

fn capabilities(cc: &eframe::CreationContext<'_>, gl: &glow::Context) -> Value {
    let mut extensions = serde_json::Map::new();
    for name in [
        "GL_EXT_memory_object",
        "GL_EXT_memory_object_win32",
        "GL_EXT_semaphore",
        "GL_EXT_semaphore_win32",
        "GL_EXT_win32_keyed_mutex",
        "GL_ARB_sync",
        "GL_ARB_texture_storage",
    ] {
        extensions.insert(name.into(), json!(gl.supported_extensions().contains(name)));
    }
    let mut procs = serde_json::Map::new();
    if let Some(get) = &cc.get_proc_address {
        for name in [
            "wglDXOpenDeviceNV",
            "wglDXCloseDeviceNV",
            "wglDXRegisterObjectNV",
            "wglDXUnregisterObjectNV",
            "wglDXLockObjectsNV",
            "wglDXUnlockObjectsNV",
            "glImportMemoryWin32HandleEXT",
            "glTexStorageMem2DEXT",
            "glAcquireKeyedMutexWin32EXT",
            "glReleaseKeyedMutexWin32EXT",
        ] {
            let symbol = std::ffi::CString::new(name).unwrap();
            procs.insert(name.into(), json!(valid_proc(get(&symbol))));
        }
    }
    json!({"glMajor": gl.version().major, "glMinor": gl.version().minor,
        "glEmbedded": gl.version().is_embedded, "extensions": extensions,
        "entryPointsPresent": procs, "wgl": wgl_capabilities(cc),
        "nativeD3D11InteropExercised": false})
}

fn valid_proc(pointer: *const std::ffi::c_void) -> bool {
    ![0, 1, 2, 3, usize::MAX].contains(&(pointer as usize))
}

#[cfg(windows)]
fn wgl_capabilities(cc: &eframe::CreationContext<'_>) -> Value {
    use std::ffi::{CStr, c_char, c_void};
    #[link(name = "opengl32")]
    unsafe extern "system" {
        fn wglGetCurrentDC() -> *mut c_void;
    }
    let Some(get) = &cc.get_proc_address else {
        return Value::Null;
    };
    let pointer = get(c"wglGetExtensionsStringARB");
    let dc = unsafe { wglGetCurrentDC() };
    if dc.is_null() || !valid_proc(pointer) {
        return json!({"queryAvailable": false});
    }
    let query: unsafe extern "system" fn(*mut c_void) -> *const c_char =
        unsafe { std::mem::transmute(pointer) };
    let result = unsafe { query(dc) };
    if result.is_null() {
        return json!({"queryAvailable": false});
    }
    let names = unsafe { CStr::from_ptr(result) }.to_string_lossy();
    json!({"queryAvailable": true,
        "WGL_NV_DX_interop": names.split_ascii_whitespace().any(|n| n == "WGL_NV_DX_interop"),
        "WGL_NV_DX_interop2": names.split_ascii_whitespace().any(|n| n == "WGL_NV_DX_interop2")})
}

#[cfg(not(windows))]
fn wgl_capabilities(_cc: &eframe::CreationContext<'_>) -> Value {
    Value::Null
}

fn main() {
    let config = match Config::parse() {
        Ok(c) => c,
        Err(_) => std::process::exit(2),
    };
    if let Some(parent) = config.report.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        std::process::exit(2);
    }
    let report = config.report.clone();
    let mut options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default().with_inner_size([960.0, 650.0]),
        ..Default::default()
    };
    options.glow_options.vsync = config.vsync;
    if eframe::run_native(
        "FreedomStream synthetic texture probe",
        options,
        Box::new(move |cc| Ok(Box::new(Probe::new(cc, config)?))),
    )
    .is_err()
    {
        let _ = std::fs::write(report, b"{\"fault\":\"eframe initialization failed\"}");
        std::process::exit(1);
    }
}
