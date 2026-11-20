//! Explicit native layout captures. All room and media values here are fixtures.
use super::*;
use anyhow::{Result, ensure};
use std::{path::PathBuf, sync::Arc};

fn synth_stream_frame(width: usize, height: usize, variant: u8) -> Arc<egui::ColorImage> {
    let mut pixels = vec![egui::Color32::from_rgb(16, 17, 22); width * height];
    for y in 0..height {
        let fy = y as f32 / height as f32;
        for x in 0..width {
            let fx = x as f32 / width as f32;
            let color = if variant == 0 {
                // Dark IDE / creative studio scene with subtle warm amber accent window
                if y < 28 {
                    egui::Color32::from_rgb(24, 24, 29)
                } else if x < 110 {
                    egui::Color32::from_rgb(20, 20, 25)
                } else {
                    let r = (18.0 + 18.0 * fx + 12.0 * (1.0 - fy)) as u8;
                    let g = (20.0 + 14.0 * fx + 10.0 * (1.0 - fy)) as u8;
                    let b = (28.0 + 24.0 * (1.0 - fx) + 14.0 * fy) as u8;
                    egui::Color32::from_rgb(r, g, b)
                }
            } else {
                // Atmospheric warm-dusk horizon / game scene gradient
                let glow = ((1.0 - ((fx - 0.52).powi(2) * 3.0 + (fy - 0.42).powi(2) * 4.5))
                    .max(0.0))
                .powf(1.8);
                let r = (22.0 + 30.0 * (1.0 - fy) + 160.0 * glow).min(240.0) as u8;
                let g = (20.0 + 22.0 * (1.0 - fy) + 105.0 * glow).min(210.0) as u8;
                let b = (30.0 + 36.0 * fy + 40.0 * glow).min(220.0) as u8;
                egui::Color32::from_rgb(r, g, b)
            };
            pixels[y * width + x] = color;
        }
    }
    if variant == 0 {
        // Draw crisp mock code lines and a warm amber timeline card
        let bar_colors = [
            egui::Color32::from_rgb(229, 169, 60),
            egui::Color32::from_rgb(122, 167, 217),
            egui::Color32::from_rgb(166, 163, 155),
            egui::Color32::from_rgb(92, 184, 122),
        ];
        for row in 0..14 {
            let y0 = 56 + row * 22;
            let bar_w = 120 + ((row * 73) % 240);
            let x0 = 136 + ((row % 3) * 18);
            let col = bar_colors[row % bar_colors.len()];
            for y in y0..(y0 + 7).min(height) {
                for x in x0..(x0 + bar_w).min(width - 24) {
                    pixels[y * width + x] = col;
                }
            }
        }
    } else {
        // Draw clean lower terrain silhouette + subtle HUD pill
        for y in (height * 62 / 100)..height {
            for x in 0..width {
                let hill = ((x as f32 / 90.0).sin() * 18.0 + (x as f32 / 37.0).cos() * 8.0) as i32;
                if y as i32 > (height as i32 * 68 / 100) + hill {
                    pixels[y * width + x] = egui::Color32::from_rgb(16, 15, 20);
                }
            }
        }
    }
    Arc::new(egui::ColorImage {
        size: [width, height],
        source_size: egui::vec2(width as f32, height as f32),
        pixels,
    })
}

pub fn run() -> Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "room".into());
    ensure!(
        [
            "room",
            "room-streams",
            "setup",
            "roster",
            "join",
            "settings",
            "profile",
            "discord",
            "information",
            "update",
            "invitation",
            "fullscreen",
        ]
        .contains(&mode.as_str()),
        "Choose room, room-streams, setup, roster, join, settings, profile, discord, information, update, invitation or fullscreen."
    );
    let output = PathBuf::from(
        std::env::args()
            .nth(2)
            .ok_or_else(|| anyhow::anyhow!("Choose an output PNG or JPEG path."))?,
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
        let account = std::env::args().any(|arg| arg == "--signed-in").then(|| crate::account::Account {
            session_id: "layout-fixture".into(),
            user_id: "123456789012345678".into(),
            display_name: "Felipão".into(),
            expires_at: 0,
        });
        let mut inner = PublisherUi::with_account_client(None, crate::account::Client::fixture(account));
        let source = crate::sources::fixture().info;
        let second_source = crate::sources::SourceInfo {
            id: "window:studio".into(),
            label: "Visual Studio Code".into(),
            width: 2560,
            height: 1440,
        };
        let encoder = crate::encoding::Encoder { factory: "nvd3d11av1enc".into(), label: "NVIDIA NVENC AV1".into(), codec: crate::encoding::Codec::Av1 };
        inner.source_id = source.id.clone();
        inner.encoder = encoder.factory.clone();
        inner.name = "Felipão".into();
        let active_selection = inner.selection();
        let tile = Tile {
            id: "layout-fixture".into(),
            name: "Felipe".into(),
            codec: "AV1".into(),
            watching: true,
            picture: Arc::new(std::sync::Mutex::new(Some(synth_stream_frame(640, 360, 0)))),
            presentation: Arc::default(),
            frames: Arc::new(120.into()),
            received_bps: Arc::new(13_800_000.into()),
            volume: Arc::default(),
            audio: Arc::default(),
            status: String::new(),
        };
        let preview_tile = Tile {
            id: "layout-fixture-2".into(),
            name: "Marina".into(),
            codec: "HEVC".into(),
            watching: false,
            picture: Arc::new(std::sync::Mutex::new(Some(synth_stream_frame(640, 360, 1)))),
            presentation: Arc::default(),
            frames: Arc::new(45.into()),
            received_bps: Arc::new(0.into()),
            volume: Arc::default(),
            audio: Arc::default(),
            status: String::new(),
        };
        let diagnostics = &tile.presentation.diagnostics;
        diagnostics.open.store(mode == "information", Ordering::Relaxed);
        *diagnostics.rates.lock().unwrap() = Some((59.9, 59.8, 0.1));
        tile.presentation.width.store(1920, Ordering::Relaxed);
        tile.presentation.height.store(1080, Ordering::Relaxed);
        *tile.presentation.viewport.lock().unwrap() = Some((960.0, 540.0));
        *diagnostics.rtc_at.lock().unwrap() = Some(Instant::now());
        *diagnostics.rtc.lock().unwrap() = vec![("Decode time".into(), "1.24 ms/frame".into()), ("Decoder".into(), "Direct3D11 (hardware)".into())];
        *diagnostics.sender.lock().unwrap() = Some((Instant::now(), serde_json::json!({ "stale": false, "age_ms": 180, "sample": { "window_ms": 3000, "sequence": 24, "codec": "AV1", "encoder": "NVIDIA NVENC AV1", "metrics": { "requested_bps": 20_000_000, "effective_bps": 20_000_000, "sent_bps": 13_800_000, "requested_fps": 60, "encoded_fps": 59.9, "sent_fps": 59.9 } } })));
        let tiles = match mode.as_str() {
            "information" | "fullscreen" => vec![tile.clone()],
            "room-streams" | "setup" | "roster" => vec![tile.clone(), preview_tile],
            _ => vec![],
        };
        let state = if mode == "join" { Snapshot::default() } else {
            Snapshot {
                connection: "connected",
                room_name: "Friday Screening".into(),
                sources: vec![source, second_source],
                encoders: vec![encoder],
                sharing: mode != "room",
                mode: Some(crate::quality::Mode { width: 1920, height: 1080, fps: 60, bitrate: 20_000_000 }),
                active_selection: Some(active_selection),
                participants: vec!["Felipe".into(), "Marina".into(), "Lucas".into()],
                participant_ids: vec![
                    ("layout-fixture".into(), "Felipe".into()),
                    ("layout-fixture-2".into(), "Marina".into()),
                    ("p-lucas".into(), "Lucas".into()),
                ],
                tiles,
                ..Default::default()
            }
        };
        let (controller, commands) = Controller::recording_for_test(state);
        inner.controller = controller;
        inner.advanced_open = mode == "settings";
        inner.setup_open = mode == "setup";
        inner.roster_open = mode == "roster";
        if mode == "fullscreen" {
            inner.fullscreen = Some("layout-fixture".into());
            inner.fullscreen_controls_until = Instant::now() + Duration::from_secs(30);
        }
        if mode == "invitation" {
            inner.browser_invitation_open = true;
            inner.invitation = "https://podetelar.example/r/friday-screening#invite=demo".into();
        }
        if mode == "profile" || mode == "discord" { inner.open_profile(); }
        if mode == "discord" { inner.profile_page = profile::SettingsPage::Discord; }
        if mode == "update" {
            use crate::update_protocol::{Phase, Status};
            inner.updates = crate::updates::Client::fixture(Status {
                phase: if std::env::args().any(|arg| arg == "--ready") { Phase::Ready } else if std::env::args().any(|arg| arg == "--downloading") { Phase::Downloading } else { Phase::Available },
                version: "0.1.5".into(),
                notes: if english {
                    "## What's new\n\n- **Streamlined onboarding** with Discord sign-in first and room invitations below.\n- **Markdown release notes** with headings, **bold**, *italic*, and `inline code` support.\n- Cleaner multi-stream previews without redundant caption clutter.\n\n### Fixes\n\n- Improved `Escape` key precedence while in fullscreen.\n- Fixed active source label display when editing broadcast settings.\n\n---\n\nSee [release notes](https://github.com/podetelar/podetelar/releases) for full details."
                } else {
                    "## Novidades\n\n- **Entrada simplificada** com login pelo Discord em destaque e convite de sala abaixo.\n- **Notas em Markdown** com títulos, **negrito**, *itálico* e `código em linha`.\n- Prévias de transmissão mais limpas, sem legendas redundantes.\n\n### Correções\n\n- Melhor prioridade da tecla `Esc` em tela cheia.\n- Correção na exibição da fonte ativa ao ajustar configurações.\n\n---\n\nVeja as [notas completas](https://github.com/podetelar/podetelar/releases) para mais detalhes."
                }
                .into(),
                progress: 42,
                ..Default::default()
            }).0;
            inner.update_open = true;
        }
        if mode == "settings" { inner.quality.bitrate = Bitrate::Manual(25_000_000); inner.custom_bitrate = std::env::args().any(|arg| arg == "--custom"); }
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
                image::save_buffer(
                    output,
                    &rgb,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgb8,
                )
                .expect("Write native screenshot");
                finished.store(true, Ordering::Release);
            });
        }
    }
}
