//! Explicit native layout captures. All room and media values here are fixtures.
use super::*;
use anyhow::{Result, ensure};
use std::{path::PathBuf, sync::Arc};

pub fn run() -> Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "room".into());
    ensure!(
        ["room", "settings", "information"].contains(&mode.as_str()),
        "Choose room, settings or information."
    );
    let output = PathBuf::from(
        std::env::args()
            .nth(2)
            .ok_or_else(|| anyhow::anyhow!("Choose an output JPEG path."))?,
    );
    ensure!(
        output.is_absolute() && !output.exists(),
        "Choose a new absolute output path."
    );
    let narrow = std::env::args().any(|arg| arg == "--narrow");
    let english = std::env::args().any(|arg| arg == "--english");
    crate::locale::select(!english);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(if narrow {
            [960.0, 540.0]
        } else {
            [1280.0, 720.0]
        }),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native("PodeTelar · layout fixture", options, Box::new(move |cc| {
        configure_theme(&cc.egui_ctx);
        let mut inner = PublisherUi::with_browser_link(None);
        let source = crate::sources::fixture().info;
        let encoder = crate::encoding::Encoder { factory: "nvd3d11av1enc".into(), label: "NVIDIA NVENC AV1".into(), codec: crate::encoding::Codec::Av1 };
        inner.source_id = source.id.clone();
        inner.encoder = encoder.factory.clone();
        inner.name = "Felipão".into();
        let active_selection = inner.selection();
        let tile = Tile { id: "layout-fixture".into(), name: "Felipe".into(), codec: "AV1".into(), watching: true, picture: Arc::default(), presentation: Arc::default(), frames: Arc::default(), received_bps: Arc::new(13_800_000.into()), volume: Arc::default(), audio: Arc::default(), status: String::new() };
        let diagnostics = &tile.presentation.diagnostics;
        diagnostics.open.store(mode == "information", Ordering::Relaxed);
        *diagnostics.rates.lock().unwrap() = Some((59.9, 59.8, 0.1));
        tile.presentation.width.store(1920, Ordering::Relaxed);
        tile.presentation.height.store(1080, Ordering::Relaxed);
        *tile.presentation.viewport.lock().unwrap() = Some((960.0, 540.0));
        *diagnostics.rtc_at.lock().unwrap() = Some(Instant::now());
        *diagnostics.rtc.lock().unwrap() = vec![("Decode time".into(), "1.24 ms/frame".into()), ("Decoder".into(), "Direct3D11 (hardware)".into())];
        *diagnostics.sender.lock().unwrap() = Some((Instant::now(), serde_json::json!({ "stale": false, "age_ms": 180, "sample": { "window_ms": 3000, "sequence": 24, "codec": "AV1", "encoder": "NVIDIA NVENC AV1", "metrics": { "requested_bps": 20_000_000, "effective_bps": 20_000_000, "sent_bps": 13_800_000, "requested_fps": 60, "encoded_fps": 59.9, "sent_fps": 59.9 } } })));
        let state = Snapshot { connection: "connected", room_name: "Friends".into(), sources: vec![source], encoders: vec![encoder], sharing: true, mode: Some(crate::quality::Mode { width: 1920, height: 1080, fps: 60, bitrate: 20_000_000 }), active_selection: Some(active_selection), tiles: if mode == "information" { vec![tile] } else { vec![] }, ..Default::default() };
        let (controller, commands) = Controller::recording_for_test(state);
        inner.controller = controller;
        inner.advanced_open = mode == "settings";
        if mode == "settings" { inner.quality.bitrate = Bitrate::Manual(25_000_000); }
        Ok(Box::new(Capture { inner, _commands: Box::new(commands), output, started: Instant::now(), requested: false, finished: Arc::new(std::sync::atomic::AtomicBool::new(false)) }))
    })).map_err(|error| anyhow::anyhow!("Native layout capture failed: {error}"))
}
struct Capture {
    inner: PublisherUi,
    _commands: Box<dyn FnMut() -> Option<Command>>,
    output: PathBuf,
    started: Instant,
    requested: bool,
    finished: Arc<std::sync::atomic::AtomicBool>,
}
impl eframe::App for Capture {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.finished.load(Ordering::Acquire) || self.started.elapsed() > Duration::from_secs(15)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(50));
    }
    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.inner.render(root, frame);
        if !self.requested && self.started.elapsed() > Duration::from_millis(800) {
            self.requested = true;
            let output = self.output.clone();
            let finished = self.finished.clone();
            root.ctx().request_screenshot(move |image| {
                let rgb: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| [pixel.r(), pixel.g(), pixel.b()])
                    .collect();
                image::save_buffer_with_format(
                    output,
                    &rgb,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgb8,
                    image::ImageFormat::Jpeg,
                )
                .expect("Write native screenshot");
                finished.store(true, Ordering::Release);
            });
        }
    }
}
