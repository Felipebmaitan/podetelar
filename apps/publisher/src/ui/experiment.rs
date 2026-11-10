//! Explicit local benchmark feature; excluded from the distributed default build.
use super::*;
use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashSet, path::PathBuf};
#[path = "micro.rs"]
mod microbench;
pub use microbench::run as micro;
pub use microbench::sdk_soak;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    invitation_url: String,
    expected_streams: usize,
    report: PathBuf,
    #[serde(default = "phase_seconds")]
    phase_seconds: u64,
    #[serde(default)]
    empty_room: bool,
    #[serde(default)]
    publish_fixture: bool,
}
fn phase_seconds() -> u64 {
    10
}

pub fn run(request: Request) -> Result<()> {
    ensure!(
        request.expected_streams <= 4 && (5..=20).contains(&request.phase_seconds),
        "Invalid local benchmark workload."
    );
    ensure!(
        request.report.is_absolute() && !request.report.exists(),
        "Choose a new absolute report path."
    );
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([960.0, 540.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "FreedomStream local application benchmark",
        options,
        Box::new(move |context| {
            configure_theme(&context.egui_ctx);
            let mut inner = PublisherUi::new();
            inner.store = None;
            inner.invitation = request.invitation_url.clone();
            inner.name = "Local benchmark".into();
            inner.controller.send(Command::Join {
                invitation: request.invitation_url.clone(),
                name: inner.name.clone(),
                route: Route::Auto,
            });
            inner.download_budget = Some(200_000_000);
            inner.controller.send(Command::DownloadBudget {
                bps: inner.download_budget,
            });
            Ok(Box::new(BenchmarkUi {
                inner,
                request,
                started: Instant::now(),
                ready: None,
                last_drive: Instant::now() - Duration::from_secs(1),
                last_sample: Instant::now(),
                phase: usize::MAX,
                samples: Vec::new(),
                watched: HashSet::new(),
                stopped: false,
                ui_calls: 0,
                ui_work_ns: 0,
                frame_cpu: 0.0,
                fault: None,
                publishing_started: false,
            }))
        }),
    )
    .map_err(|_| anyhow::anyhow!("Cannot run native application benchmark."))?;
    Ok(())
}

struct BenchmarkUi {
    inner: PublisherUi,
    request: Request,
    started: Instant,
    ready: Option<Instant>,
    last_drive: Instant,
    last_sample: Instant,
    phase: usize,
    samples: Vec<serde_json::Value>,
    watched: HashSet<String>,
    stopped: bool,
    ui_calls: u64,
    ui_work_ns: u64,
    frame_cpu: f64,
    fault: Option<&'static str>,
    publishing_started: bool,
}
const PHASES: [&str; 6] = [
    "grid",
    "focus",
    "fullscreen",
    "minimized",
    "restore",
    "stopped",
];

impl BenchmarkUi {
    fn drive(&mut self, ctx: &egui::Context) {
        if self.last_drive.elapsed() < Duration::from_millis(100) {
            return;
        }
        self.last_drive = Instant::now();
        let state = self.inner.controller.snapshot();
        if self.request.publish_fixture
            && !self.publishing_started
            && state.connection == "connected"
        {
            self.publishing_started = true;
            self.inner.controller.send(Command::Start {
                source_id: "fixture".into(),
                encoder: "nvd3d11h265enc".into(),
                audio: crate::audio::Choice::Off,
                exclusions: Vec::new(),
                quality: Settings {
                    resolution: quality::Resolution::P1080,
                    fps: 60,
                    profile: quality::Profile::Motion,
                    bitrate: quality::Bitrate::Manual(6_000_000),
                },
            });
        }
        if !self.stopped && !self.request.empty_room && state.connection == "connected" {
            if let Some(tile) = state
                .tiles
                .iter()
                .find(|tile| !self.watched.contains(&tile.id))
            {
                self.inner.controller.send(Command::Watch {
                    id: tile.id.clone(),
                    enabled: true,
                });
                self.watched.insert(tile.id.clone());
            }
        }
        let active = state
            .tiles
            .iter()
            .filter(|tile| tile.watching && tile.frames.load(Ordering::Relaxed) > 10)
            .count();
        if self.ready.is_none()
            && state.connection == "connected"
            && !state.testing_connection
            && (!self.request.publish_fixture || state.sharing)
            && (self.request.empty_room || active >= self.request.expected_streams)
        {
            self.ready = Some(Instant::now() + Duration::from_secs(3));
        }
        if self.ready.is_none() && self.started.elapsed() > Duration::from_secs(25) {
            self.fault = Some("readiness_timeout");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let Some(ready) = self.ready else {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        };
        let Some(elapsed) = Instant::now().checked_duration_since(ready) else {
            return;
        };
        let phase = (elapsed.as_secs() / self.request.phase_seconds) as usize;
        if phase >= PHASES.len() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if phase != self.phase {
            self.phase = phase;
            let first = state
                .tiles
                .iter()
                .find(|tile| tile.watching)
                .map(|tile| tile.id.clone());
            match PHASES[phase] {
                "grid" => {
                    self.inner.focused = None;
                    self.inner.set_fullscreen(ctx, None);
                }
                "focus" => self.inner.focused = first,
                "fullscreen" => self.inner.set_fullscreen(ctx, first),
                "minimized" => {
                    self.inner.set_fullscreen(ctx, None);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                "restore" => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    self.inner.focused = None;
                }
                "stopped" => {
                    for tile in &state.tiles {
                        self.inner.controller.send(Command::Watch {
                            id: tile.id.clone(),
                            enabled: false,
                        });
                    }
                    self.stopped = true;
                }
                _ => {}
            }
        }
        if self.last_sample.elapsed() >= Duration::from_secs(1) {
            let texture_bytes: usize = self
                .inner
                .textures
                .values()
                .map(|texture| texture.size()[0] * texture.size()[1] * 4)
                .sum();
            self.samples.push(json!({"second":elapsed.as_secs_f64(),"phase":PHASES[phase],"connection":state.connection,"activeWatches":state.tiles.iter().filter(|tile|tile.watching).count(),"convertedFrames":state.tiles.iter().map(|tile|tile.presentation.converted.load(Ordering::Relaxed)).collect::<Vec<_>>(),"uploadedFrames":state.tiles.iter().map(|tile|tile.presentation.uploaded.load(Ordering::Relaxed)).collect::<Vec<_>>(),"nativeLogMessages":crate::playback::SDK_MESSAGES.load(Ordering::Relaxed),"sharing":state.sharing,"encodedFrames":state.metrics.frames,"previewFrames":state.metrics.preview_frames,"frames":state.tiles.iter().map(|tile|tile.frames.load(Ordering::Relaxed)).collect::<Vec<_>>(),"videoBps":state.tiles.iter().map(|tile|tile.received_bps.load(Ordering::Relaxed)).sum::<u64>(),"textureBytes":texture_bytes,"textures":self.inner.textures.len(),"uiCalls":self.ui_calls,"uiWorkMs":self.ui_work_ns as f64/1e6,"reportedFrameCpuSeconds":self.frame_cpu,"errorPresent":!state.error.is_empty(),"streamErrorCount":state.tiles.iter().filter(|tile|!tile.status.is_empty()).count()}));
            self.last_sample = Instant::now();
        }
        if PHASES[phase] == "minimized" {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}
impl eframe::App for BenchmarkUi {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drive(ctx);
    }
    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        let now = Instant::now();
        self.inner.ui(root, frame);
        self.ui_work_ns += now.elapsed().as_nanos() as u64;
        self.ui_calls += 1;
        self.frame_cpu += frame.info().cpu_usage.unwrap_or_default() as f64;
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.inner.controller.send(Command::Shutdown);
        let completed = self.fault.is_none()
            && self.ready.is_some_and(|ready| {
                Instant::now()
                    .checked_duration_since(ready)
                    .is_some_and(|elapsed| {
                        elapsed.as_secs() >= self.request.phase_seconds * PHASES.len() as u64
                    })
            });
        let report = json!({"schema":1,"kind":"actual-native-ui-room","completed":completed,"fault":self.fault,"expectedStreams":self.request.expected_streams,"phaseSeconds":self.request.phase_seconds,"wallSeconds":self.started.elapsed().as_secs_f64(),"samples":self.samples});
        if let Some(parent) = self.request.report.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(
            &self.request.report,
            serde_json::to_vec_pretty(&report).unwrap_or_default(),
        );
    }
}
