//! Explicit native layout captures. All room and media values here are fixtures.
use super::*;
use anyhow::{Result, ensure};
use std::{path::PathBuf, sync::Arc};

pub fn run() -> Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "room".into());
    ensure!(
        [
            "room",
            "join",
            "settings",
            "profile",
            "discord",
            "information",
            "update",
            "golive",
            "tour"
        ]
        .contains(&mode.as_str()),
        "Choose room, join, settings, profile, discord, information, update or golive."
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
    if mode == "tour" {
        return tour(output);
    }
    let narrow = std::env::args().any(|arg| arg == "--narrow");
    let english = std::env::args().any(|arg| arg == "--english");
    crate::locale::select(!english);
    if let Some(name) = std::env::args().skip_while(|arg| arg != "--theme").nth(1) {
        crate::theme::select(match name.as_str() {
            "light" => crate::theme::Theme::Light,
            "ruby" | "comunista" => crate::theme::Theme::Ruby,
            "black" => crate::theme::Theme::Black,
            "emerald" | "patriota" => crate::theme::Theme::Emerald,
            _ => crate::theme::Theme::Classic,
        });
    }
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
        let mut state = if mode == "join" { Snapshot::default() } else {
            Snapshot { connection: "connected", room_name: "Friends".into(), sources: vec![source], encoders: vec![encoder], sharing: true, mode: Some(crate::quality::Mode { width: 1920, height: 1080, fps: 60, bitrate: 20_000_000 }), active_selection: Some(active_selection), tiles: if mode == "information" { vec![tile] } else { vec![] }, ..Default::default() }
        };
        if let Some(scene) = std::env::args().skip_while(|arg| arg != "--scene").nth(1) {
            // Gradient pictures stand in for real video, so layouts can be judged by eye.
            let gradient = |seed: u8| -> egui::ColorImage {
                let (w, h) = (640usize, 360usize);
                let mut pixels = Vec::with_capacity(w * h);
                for y in 0..h {
                    for x in 0..w {
                        let t = x as f32 / w as f32;
                        let u = y as f32 / h as f32;
                        let hue = (seed as f32 * 0.23 + t * 0.35 + u * 0.15).fract();
                        let (r, g, b) = (
                            (0.5 + 0.5 * (hue * 6.28).sin()) * (0.35 + 0.5 * (1.0 - u)),
                            (0.5 + 0.5 * (hue * 6.28 + 2.1).sin()) * (0.35 + 0.5 * (1.0 - u)),
                            (0.5 + 0.5 * (hue * 6.28 + 4.2).sin()) * (0.45 + 0.5 * u),
                        );
                        pixels.push(egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8));
                    }
                }
                egui::ColorImage { size: [w, h], source_size: egui::vec2(w as f32, h as f32), pixels }
            };
            let make = |id: &str, name: &str, watching: bool, seed: u8| -> Tile {
                let picture: crate::playback::Picture = Arc::default();
                *picture.lock().unwrap() = Some(Arc::new(gradient(seed)));
                Tile {
                    id: id.into(),
                    name: name.into(),
                    codec: "AV1".into(),
                    watching,
                    picture,
                    presentation: Arc::default(),
                    frames: Arc::new(60.into()),
                    received_bps: Arc::new(9_400_000.into()),
                    volume: Arc::new(72.into()),
                    audio: Arc::default(),
                    status: String::new(),
                }
            };
            state.participants = vec!["Ana".into(), "Bruno".into(), "Camila".into(), "Diego".into()];
            state.participant_ids = vec![
                ("id-a".into(), "Ana".into()),
                ("id-b".into(), "Bruno".into()),
                ("id-c".into(), "Camila".into()),
                ("id-d".into(), "Diego".into()),
            ];
            let tiles = match scene.as_str() {
                "one" => vec![make("a", "Ana", true, 1)],
                "many" | "spot" => vec![
                    make("a", "Ana", true, 1),
                    make("b", "Bruno", false, 2),
                    make("c", "Camila", true, 3),
                    make("d", "Diego", false, 4),
                ],
                "pip" => vec![make("a", "Ana", true, 1), make("b", "Bruno", false, 2)],
                _ => vec![],
            };
            state.sharing = matches!(scene.as_str(), "mine" | "pip");
            state.tiles = tiles;
            if state.sharing {
                let mut bytes = Vec::new();
                let image = image::RgbImage::from_fn(640, 360, |x, y| {
                    image::Rgb([(40 + x / 4) as u8, (60 + y / 3) as u8, 160])
                });
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 80)
                    .encode_image(&image)
                    .expect("Encode fixture preview");
                *state.self_preview.lock().unwrap() = Some(crate::preview::Frame { sequence: 1, bytes });
            }
            if scene == "spot" {
                inner.view_mode = ViewMode::Spotlight;
            }
        }
        if std::env::args().any(|arg| arg == "--error") {
            state.error = "Could not reach the server. Check your connection and try again.".into();
        }
        if std::env::args().any(|arg| arg == "--busy") {
            state.busy = true;
            state.operation = "Starting your stream…";
        }
        let (controller, commands) = Controller::recording_for_test(state);
        inner.controller = controller;
        inner.advanced_open = mode == "settings";
        if mode == "golive" { inner.go_live = Some(false); }
        if mode == "profile" || mode == "discord" { inner.open_profile(); }
        if mode == "discord" { inner.profile_page = profile::SettingsPage::Discord; }
        if mode == "update" {
            use crate::update_protocol::{Phase, Status};
            inner.updates = crate::updates::Client::fixture(Status {
                phase: if std::env::args().any(|arg| arg == "--ready") { Phase::Ready } else if std::env::args().any(|arg| arg == "--downloading") { Phase::Downloading } else { Phase::Available },
                version: "0.1.3".into(),
                notes: if english { "A simpler update experience\n\n• Removed the redundant sidebar refresh button.\n• A soft glow lets you know when an update is available.\n• Read the changelog here before downloading.\n\nYour sharing and watching continue until you choose to restart." } else { "Uma experiência de atualização mais simples\n\n• Removido o botão de atualizar da barra lateral.\n• Um brilho suave avisa quando há uma atualização.\n• Leia as novidades antes de baixar.\n\nSuas transmissões continuam até você escolher reiniciar." }.into(),
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
        if !self.requested && self.started.elapsed() > Duration::from_millis(1800) {
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

// ---------------------------------------------------------------------------
// Guided tour: scripted states rendered in real time, one JPEG per video frame.
// Nothing here touches the network, devices or the desktop; the frames come from
// egui's own framebuffer, so other windows can never end up in the recording.
// ---------------------------------------------------------------------------

const TOUR_FPS: f32 = 30.0;
const TOUR_END: f32 = 90.0;
const TOUR_INVITATION: &str = "https://podetelar.example/r/sala#invite=demo-demo-demo";

fn tour_gradient(seed: u8) -> egui::ColorImage {
    let (w, h) = (640usize, 360usize);
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let t = x as f32 / w as f32;
            let u = y as f32 / h as f32;
            let hue = (seed as f32 * 0.23 + t * 0.35 + u * 0.15).fract();
            let shade = 0.35 + 0.5 * (1.0 - u);
            let c =
                |phase: f32, k: f32| ((0.5 + 0.5 * (hue * 6.28 + phase).sin()) * k * 255.0) as u8;
            pixels.push(egui::Color32::from_rgb(
                c(0.0, shade),
                c(2.1, shade),
                c(4.2, 0.45 + 0.5 * u),
            ));
        }
    }
    egui::ColorImage {
        size: [w, h],
        source_size: egui::vec2(w as f32, h as f32),
        pixels,
    }
}

fn tour_tile(id: &str, name: &str, watching: bool, seed: u8) -> Tile {
    let picture: crate::playback::Picture = Arc::default();
    *picture.lock().unwrap() = Some(Arc::new(tour_gradient(seed)));
    Tile {
        id: id.into(),
        name: name.into(),
        codec: "AV1".into(),
        watching,
        picture,
        presentation: Arc::default(),
        frames: Arc::new(60.into()),
        received_bps: Arc::new(9_400_000.into()),
        volume: Arc::new(72.into()),
        audio: Arc::default(),
        status: String::new(),
    }
}

fn tour_state(inner: &PublisherUi, scene: &str) -> Snapshot {
    if scene == "join" {
        return Snapshot::default();
    }
    let source = crate::sources::fixture().info;
    let encoder = crate::encoding::Encoder {
        factory: "nvd3d11av1enc".into(),
        label: "NVIDIA NVENC AV1".into(),
        codec: crate::encoding::Codec::Av1,
    };
    let sharing = matches!(scene, "mine" | "pip");
    let tiles = match scene {
        "one" => vec![tour_tile("a", "Ana", true, 1)],
        "many" => vec![
            tour_tile("a", "Ana", true, 1),
            tour_tile("b", "Bruno", false, 2),
            tour_tile("c", "Camila", true, 3),
            tour_tile("d", "Elisa", false, 4),
        ],
        "pip" => vec![
            tour_tile("a", "Ana", true, 1),
            tour_tile("b", "Bruno", false, 2),
        ],
        _ => vec![],
    };
    let state = Snapshot {
        connection: "connected",
        room_name: "Sala da Galera".into(),
        sources: vec![source],
        encoders: vec![encoder],
        sharing,
        mode: Some(crate::quality::Mode {
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate: 20_000_000,
        }),
        active_selection: Some(inner.selection()),
        tiles,
        participants: vec![
            "Ana".into(),
            "Bruno".into(),
            "Camila".into(),
            "Elisa".into(),
        ],
        participant_ids: vec![
            ("id-a".into(), "Ana".into()),
            ("id-b".into(), "Bruno".into()),
            ("id-c".into(), "Camila".into()),
            ("id-d".into(), "Elisa".into()),
        ],
        ..Default::default()
    };
    if sharing {
        let mut bytes = Vec::new();
        let image = image::RgbImage::from_fn(640, 360, |x, y| {
            image::Rgb([(40 + x / 4) as u8, (60 + y / 3) as u8, 160])
        });
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 80)
            .encode_image(&image)
            .expect("Encode fixture preview");
        *state.self_preview.lock().unwrap() = Some(crate::preview::Frame { sequence: 1, bytes });
    }
    state
}

enum TourStep {
    Scene(&'static str),
    Invitation(usize),
    Name(&'static str),
    Island(bool),
    Hover(Option<(f32, f32)>),
    View(ViewMode, Option<&'static str>),
    GoLive(bool),
    Advanced(bool),
    Theme(crate::theme::Theme),
    Update(bool),
}

struct TourApp {
    inner: PublisherUi,
    commands: Vec<Box<dyn FnMut() -> Option<Command>>>,
    dir: PathBuf,
    steps: Vec<(f32, TourStep)>,
    next: usize,
    hover: Option<egui::Pos2>,
    hovering: bool,
    frame: i64,
}

fn tour_steps() -> Vec<(f32, TourStep)> {
    use crate::theme::Theme;
    let mut steps = vec![(0.0, TourStep::Scene("join")), (0.0, TourStep::Name(""))];
    for k in 1..=TOUR_INVITATION.len() {
        steps.push((1.6 + k as f32 * 0.05, TourStep::Invitation(k)));
    }
    for (k, name) in ["D", "Di", "Die", "Dieg", "Diego"].into_iter().enumerate() {
        steps.push((5.0 + k as f32 * 0.2, TourStep::Name(name)));
    }
    steps.extend([
        (7.0, TourStep::Scene("empty")),
        (11.5, TourStep::Island(true)),
        (15.5, TourStep::Island(false)),
        (16.5, TourStep::Scene("one")),
        (17.5, TourStep::Hover(Some((640.0, 330.0)))),
        (22.0, TourStep::Scene("many")),
        (23.0, TourStep::Hover(Some((330.0, 250.0)))),
        (27.0, TourStep::Hover(None)),
        (27.5, TourStep::View(ViewMode::Spotlight, None)),
        (32.5, TourStep::View(ViewMode::Grid, Some("a"))),
        (36.5, TourStep::View(ViewMode::Grid, None)),
        (37.0, TourStep::Scene("mine")),
        (42.0, TourStep::Scene("pip")),
        (47.0, TourStep::GoLive(true)),
        (51.5, TourStep::GoLive(false)),
        (52.0, TourStep::Advanced(true)),
        (55.0, TourStep::Theme(Theme::Black)),
        (58.0, TourStep::Theme(Theme::Light)),
        (61.0, TourStep::Theme(Theme::Ruby)),
        (64.0, TourStep::Theme(Theme::Emerald)),
        (67.0, TourStep::Theme(Theme::Classic)),
        (69.5, TourStep::Advanced(false)),
        (70.0, TourStep::Scene("many")),
        (70.5, TourStep::Theme(Theme::Emerald)),
        (73.5, TourStep::Theme(Theme::Ruby)),
        (76.5, TourStep::Theme(Theme::Black)),
        (79.5, TourStep::Theme(Theme::Light)),
        (82.0, TourStep::Theme(Theme::Classic)),
        (83.0, TourStep::Scene("one")),
        (84.0, TourStep::Update(true)),
    ]);
    steps.sort_by(|a, b| a.0.total_cmp(&b.0));
    steps
}

fn tour(dir: PathBuf) -> Result<()> {
    std::fs::create_dir_all(&dir)?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 720.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    let result = eframe::run_native(
        "PodeTelar · tour",
        options,
        Box::new(move |cc| {
            crate::theme::select(crate::theme::Theme::Classic);
            configure_theme(&cc.egui_ctx);
            let mut inner = PublisherUi::with_browser_link(None);
            let source = crate::sources::fixture().info;
            inner.source_id = source.id.clone();
            inner.encoder = "nvd3d11av1enc".into();
            inner.name = String::new();
            let (controller, command) = Controller::recording_for_test(tour_state(&inner, "join"));
            inner.controller = controller;
            Ok(Box::new(TourApp {
                inner,
                commands: vec![Box::new(command)],
                dir,
                steps: tour_steps(),
                next: 0,
                hover: None,
                hovering: false,
                frame: 0,
            }))
        }),
    );
    result.map_err(|error| anyhow::anyhow!("Native tour failed: {error}"))
}

impl eframe::App for TourApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.frame as f32 / TOUR_FPS > TOUR_END {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // Virtual time: every rendered frame is exactly one video frame, however slow the render.
        self.frame += 1;
        raw_input.time = Some(self.frame as f64 / TOUR_FPS as f64);
        raw_input.predicted_dt = 1.0 / TOUR_FPS;
        if let Some(pos) = self.hover {
            raw_input.events.push(egui::Event::PointerMoved(pos));
            self.hovering = true;
        } else if self.hovering {
            raw_input.events.push(egui::Event::PointerGone);
            self.hovering = false;
        }
    }
    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let now = self.frame as f32 / TOUR_FPS;
        while self.next < self.steps.len() && self.steps[self.next].0 <= now {
            match &self.steps[self.next].1 {
                TourStep::Scene(name) => {
                    let (controller, command) =
                        Controller::recording_for_test(tour_state(&self.inner, name));
                    self.inner.controller = controller;
                    self.commands.push(Box::new(command));
                    self.inner.pip_hidden = false;
                }
                TourStep::Invitation(len) => {
                    self.inner.invitation = TOUR_INVITATION.chars().take(*len).collect();
                }
                TourStep::Name(name) => self.inner.name = (*name).into(),
                TourStep::Island(open) => self.inner.island_open = *open,
                TourStep::Hover(at) => self.hover = at.map(|(x, y)| egui::pos2(x, y)),
                TourStep::View(mode, focus) => {
                    self.inner.view_mode = *mode;
                    self.inner.focused = focus.map(|id| id.to_owned());
                }
                TourStep::GoLive(open) => self.inner.go_live = open.then_some(false),
                TourStep::Advanced(open) => self.inner.advanced_open = *open,
                TourStep::Theme(theme) => {
                    crate::theme::select(*theme);
                    apply_theme(&ctx);
                }
                TourStep::Update(open) => {
                    if *open {
                        use crate::update_protocol::{Phase, Status};
                        self.inner.updates = crate::updates::Client::fixture(Status {
                            phase: Phase::Available,
                            version: "0.1.5".into(),
                            notes: "Novo visual

• Palco: a janela inteira é a transmissão.
• Cinco temas e troca de idioma.
• Volume e modos grade, destaque e foco."
                                .into(),
                            progress: 0,
                            ..Default::default()
                        })
                        .0;
                        self.inner.update_open = true;
                    }
                }
            }
            self.next += 1;
        }
        self.inner.render(root, frame);
        let _ = now;
        {
            let path = self.dir.join(format!("frame_{:05}.jpg", self.frame));
            ctx.request_screenshot(move |image| {
                let rgb: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|p| [p.r(), p.g(), p.b()])
                    .collect();
                if let Ok(file) = std::fs::File::create(path) {
                    let mut writer = std::io::BufWriter::new(file);
                    let _ = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 88)
                        .encode(
                            &rgb,
                            image.width() as u32,
                            image.height() as u32,
                            image::ExtendedColorType::Rgb8,
                        );
                }
            });
        }
    }
}
