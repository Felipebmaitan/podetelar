//! Explicit local benchmark feature; excluded from the distributed default build.
use super::*;
use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashSet, path::PathBuf};
#[path = "micro.rs"]
mod microbench;
// These entry points are consumed by the standalone experiment binary, while
// the shared UI module is also compiled by the publisher and its test target.
#[allow(unused_imports)]
pub use microbench::run as micro;
#[allow(unused_imports)]
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
    #[serde(default)]
    disconnected: bool,
    #[serde(default = "one_round")]
    rounds: u64,
    #[serde(default)]
    steady_seconds: u64,
}
impl Request {
    fn total_seconds(&self) -> u64 {
        self.phase_seconds * PHASES.len() as u64 * self.rounds + self.steady_seconds
    }
    fn step(&self, elapsed: u64) -> usize {
        if elapsed < self.phase_seconds + self.steady_seconds {
            0
        } else {
            (1 + (elapsed - self.phase_seconds - self.steady_seconds) / self.phase_seconds) as usize
        }
    }
}
fn one_round() -> u64 {
    1
}
fn phase_seconds() -> u64 {
    10
}

pub fn run(request: Request) -> Result<()> {
    ensure!(
        request.expected_streams <= 5
            && (5..=20).contains(&request.phase_seconds)
            && (1..=7).contains(&request.rounds)
            && request.steady_seconds <= 360
            && (request.steady_seconds == 0 || request.rounds == 1),
        "Invalid local benchmark workload."
    );
    ensure!(
        request.report.is_absolute() && !request.report.exists(),
        "Choose a new absolute report path."
    );
    let narrow = std::env::var("FREEDOMSTREAM_DEEP_NARROW").is_ok_and(|v| v == "1");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(if narrow {
                [960.0, 540.0]
            } else {
                [1280.0, 720.0]
            })
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
            if !request.disconnected {
                inner.controller.send(Command::Join {
                    invitation: request.invitation_url.clone(),
                    name: inner.name.clone(),
                    route: Route::Auto,
                });
            }
            inner.download_budget = Some(200_000_000);
            inner.controller.send(Command::DownloadBudget {
                bps: inner.download_budget,
            });
            Ok(Box::new(BenchmarkUi {
                inner,
                request,
                started: Instant::now(),
                started_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
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
                frame_intervals: std::array::from_fn(|_| Vec::new()),
                last_ui_at: None,
                heap_actions: Vec::new(),
                heap_cycle: None,
                cycle_started: Instant::now(),
                first_handoffs: Vec::new(),
                handoff_cycle: None,
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
    started_unix_ms: u64,
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
    frame_intervals: [Vec<f64>; 6],
    last_ui_at: Option<Instant>,
    heap_actions: Vec<serde_json::Value>,
    heap_cycle: Option<usize>,
    cycle_started: Instant,
    first_handoffs: Vec<serde_json::Value>,
    handoff_cycle: Option<usize>,
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
        if !self.stopped
            && !self.request.empty_room
            && state.connection == "connected"
            && let Some(tile) = state
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
        let active = state
            .tiles
            .iter()
            .filter(|tile| tile.watching && tile.frames.load(Ordering::Relaxed) > 10)
            .count();
        if self.ready.is_none()
            && (self.request.disconnected
                || (state.connection == "connected"
                    && !state.testing_connection
                    && (!self.request.publish_fixture || state.sharing)
                    && (self.request.empty_room || active >= self.request.expected_streams)))
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
        if std::env::var("FREEDOMSTREAM_DEEP_SCROLL").is_ok_and(|v| v == "1") {
            let second = elapsed.as_secs() % self.request.phase_seconds;
            let offset = match second {
                0..=2 => 0.0f32,
                3..=5 => 400.0,
                6..=8 => 800.0,
                _ => 1400.0,
            };
            ctx.data_mut(|data| data.insert_temp(egui::Id::unique("deep-scroll-offset"), offset));
        }
        let step = self.request.step(elapsed.as_secs());
        if step >= PHASES.len() * self.request.rounds as usize {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let phase = step % PHASES.len();
        let cycle = step / PHASES.len();
        if phase != self.phase {
            self.phase = phase;
            self.last_ui_at = None;
            let first = state
                .tiles
                .iter()
                .find(|tile| tile.watching)
                .map(|tile| tile.id.clone());
            match PHASES[phase] {
                "grid" => {
                    self.cycle_started = Instant::now();
                    if cycle > 0 {
                        self.stopped = false;
                        self.watched.clear();
                        if cycle.is_multiple_of(2) {
                            self.inner.controller.send(Command::Leave);
                            self.inner.controller.send(Command::Join {
                                invitation: self.request.invitation_url.clone(),
                                name: "Local benchmark".into(),
                                route: Route::Auto,
                            });
                        }
                    }
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
        if phase == 0
            && self.handoff_cycle != Some(cycle)
            && state
                .tiles
                .iter()
                .filter(|tile| {
                    tile.watching && tile.presentation.uploaded.load(Ordering::Relaxed) > 0
                })
                .count()
                >= self.request.expected_streams
        {
            self.first_handoffs.push(json!({"cycle":cycle,"rewatchToAllFirstHandoffsMs":self.cycle_started.elapsed().as_secs_f64()*1000.0,"leaveRejoin":cycle>0&&cycle.is_multiple_of(2)}));
            self.handoff_cycle = Some(cycle);
        }
        if phase == 5
            && elapsed.as_secs() % self.request.phase_seconds >= 5
            && self.heap_cycle != Some(cycle)
            && !state.sharing
            && state.tiles.iter().all(|tile| !tile.watching)
            && crate::resource_experiment::video_tasks_drained()
            && std::env::var("FREEDOMSTREAM_DEEP_HEAP").is_ok_and(|v| v == "1")
        {
            self.heap_actions.push(
                json!({"cycle":cycle,"action":crate::resource_experiment::optimize_idle_heap()}),
            );
            self.heap_cycle = Some(cycle);
        }
        if self.last_sample.elapsed() >= Duration::from_secs(1) {
            let texture_bytes: usize = self
                .inner
                .textures
                .values()
                .map(|texture| texture.size()[0] * texture.size()[1] * 4)
                .sum();
            self.samples.push(json!({"second":elapsed.as_secs_f64(),"windowFocused":ctx.input(|i|i.viewport().focused),"windowMinimized":ctx.input(|i|i.viewport().minimized),"windowOccluded":ctx.input(|i|i.viewport().occluded),"cycle":cycle,"phase":PHASES[phase],"connection":state.connection,"activeWatches":state.tiles.iter().filter(|tile|tile.watching).count(),"videoCodecs":state.tiles.iter().filter(|tile|tile.watching).map(|tile|tile.codec.clone()).collect::<Vec<_>>(),"owners":crate::resource_experiment::snapshot(),"nativeOwners":crate::resource_experiment::native_snapshot(),"planarGlUpdates":crate::planar_experiment::UPDATES.load(Ordering::Relaxed),"planarGlBytes":crate::planar_experiment::BYTES.load(Ordering::Relaxed),"planarGlDraws":crate::planar_experiment::DRAWS.load(Ordering::Relaxed),"planarGlFaults":crate::planar_experiment::FAULTS.load(Ordering::Relaxed),"planarGlUploadNs":crate::planar_experiment::UPLOAD_NS.load(Ordering::Relaxed),"planarMaxAgeUs":crate::planar_experiment::MAX_AGE_US.load(Ordering::Relaxed),"visiblePictures":state.tiles.iter().map(|tile|tile.presentation.visible.load(Ordering::Relaxed)).collect::<Vec<_>>(),"convertedFrames":state.tiles.iter().map(|tile|tile.presentation.converted.load(Ordering::Relaxed)).collect::<Vec<_>>(),"uploadedFrames":state.tiles.iter().map(|tile|tile.presentation.uploaded.load(Ordering::Relaxed)).collect::<Vec<_>>(),"subscriptionSetterCalls":crate::playback::SUBSCRIPTION_SETTERS.load(Ordering::Relaxed),"nativeLogMessages":crate::playback::SDK_MESSAGES.load(Ordering::Relaxed),"sharing":state.sharing,"encodedFrames":state.metrics.frames,"previewFrames":state.metrics.preview_frames,"frames":state.tiles.iter().map(|tile|tile.frames.load(Ordering::Relaxed)).collect::<Vec<_>>(),"audioFrames":state.tiles.iter().map(|tile|tile.audio.lock().map(|m|m.frames).unwrap_or_default()).collect::<Vec<_>>(),"audioQueueMaxMs":state.tiles.iter().map(|tile|tile.audio.lock().map(|m|m.playback_queue_max_ms).unwrap_or_default()).collect::<Vec<_>>(),"audioDroppedFrames":state.tiles.iter().map(|tile|tile.audio.lock().map(|m|m.playback_dropped_frames).unwrap_or_default()).collect::<Vec<_>>(),"videoBps":state.tiles.iter().map(|tile|tile.received_bps.load(Ordering::Relaxed)).sum::<u64>(),"textureBytes":texture_bytes,"textures":self.inner.textures.len(),"uiCalls":self.ui_calls,"uiWorkMs":self.ui_work_ns as f64/1e6,"reportedFrameCpuSeconds":self.frame_cpu,"errorPresent":!state.error.is_empty(),"streamErrorCount":state.tiles.iter().filter(|tile|!tile.status.is_empty()).count()}));
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
        if self.phase < PHASES.len() {
            if let Some(last) = self.last_ui_at {
                self.frame_intervals[self.phase]
                    .push(now.duration_since(last).as_secs_f64() * 1000.0);
            }
            self.last_ui_at = Some(now);
        }
        self.inner.ui(root, frame);
        self.ui_work_ns += now.elapsed().as_nanos() as u64;
        self.ui_calls += 1;
        self.frame_cpu += frame.info().cpu_usage.unwrap_or_default() as f64;
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.inner.planar.destroy(_gl);
        self.inner.controller.send(Command::Shutdown);
        let completed = self.fault.is_none()
            && self.ready.is_some_and(|ready| {
                Instant::now()
                    .checked_duration_since(ready)
                    .is_some_and(|elapsed| elapsed.as_secs() >= self.request.total_seconds())
            });
        let intervals:Vec<_>=self.frame_intervals.iter().enumerate().map(|(phase,values)|{
            let mut sorted=values.clone();sorted.sort_by(f64::total_cmp);let percentile=|p:f64|sorted.get(((sorted.len().saturating_sub(1))as f64*p)as usize).copied();
            json!({"phase":PHASES[phase],"count":values.len(),"p50Ms":percentile(0.5),"p95Ms":percentile(0.95),"p99Ms":percentile(0.99),"maxMs":sorted.last()})
        }).collect();
        let report = json!({"schema":1,"kind":"actual-native-ui-room","uiStartedAtUnixMs":self.started_unix_ms,"readyOffsetSeconds":self.ready.map(|ready|ready.duration_since(self.started).as_secs_f64()),"completed":completed,"fault":self.fault,"heapActions":self.heap_actions,"firstHandoffs":self.first_handoffs,"expectedStreams":self.request.expected_streams,"phaseSeconds":self.request.phase_seconds,"rounds":self.request.rounds,"steadySeconds":self.request.steady_seconds,"wallSeconds":self.started.elapsed().as_secs_f64(),"uiInvocationIntervals":intervals,"samples":self.samples});
        if let Some(parent) = self.request.report.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(
            &self.request.report,
            serde_json::to_vec_pretty(&report).unwrap_or_default(),
        );
    }
}
