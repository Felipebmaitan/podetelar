mod policy;
pub use policy::Identity as ProcessIdentity;
#[cfg(windows)]
mod windows;
#[cfg(all(windows, feature = "performance-experiments"))]
pub(crate) use windows::inventory_benchmark;

use anyhow::{Context, Result, bail, ensure};
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSink;
use livekit::webrtc::{audio_frame::AudioFrame, audio_source::native::NativeAudioSource};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "mode", content = "application", rename_all = "lowercase")]
pub enum Choice {
    #[default]
    Off,
    Desktop,
    Application(String),
    Window(ProcessIdentity),
}

#[derive(Clone, Serialize)]
pub struct Application {
    pub id: String,
    pub label: String,
}

#[derive(Default, Clone, Serialize)]
pub struct Metrics {
    pub applications: usize,
    pub frames: u64,
    pub peak: f64,
    pub channel_difference_peak: f64,
    pub tones: [f64; 3],
    pub windows: Vec<SpectralWindow>,
    // Observed pipeline backlog only. These are not physical A/V offsets.
    pub capture_age_ms: f64,
    pub capture_age_max_ms: f64,
    pub transport_wait_ms: f64,
    pub playback_queue_ms: f64,
    pub playback_queue_max_ms: f64,
    pub playback_dropped_frames: u64,
}

#[derive(Clone, Serialize)]
pub struct SpectralWindow {
    pub second: u64,
    pub tones: [f64; 3],
}

pub fn applications() -> Result<Vec<Application>> {
    #[cfg(windows)]
    {
        let owner = windows::identity(std::process::id())?;
        Ok(windows::inventory()?.applications(owner))
    }
    #[cfg(not(windows))]
    {
        bail!("Application audio is currently available on Windows only.")
    }
}

struct Pipeline {
    pipeline: gst::Pipeline,
    sink: AppSink,
    #[cfg(windows)]
    _leases: Vec<crate::win32::Handle>,
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

impl Pipeline {
    fn start(roots: &[policy::Identity], stop: &AtomicBool) -> Result<Self> {
        ensure!(
            !stop.load(Ordering::Acquire),
            "Audio capture was cancelled."
        );
        let capture = gst::ElementFactory::make("wasapi2src")
            .build()
            .context("Windows application audio is unavailable.")?;
        ensure!(
            capture.find_property("loopback-target-pid").is_some(),
            "Application audio requires Windows 11 or Windows build 20348 or newer."
        );
        let pipeline = gst::parse::launch("audiotestsrc is-live=true wave=silence samplesperbuffer=480 ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! mix. audiomixer name=mix force-live=true ignore-inactive-pads=true latency=30000000 output-buffer-duration=10000000 ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! appsink name=pcm max-buffers=4 drop=true sync=false")?.downcast::<gst::Pipeline>().map_err(|_| anyhow::anyhow!("Invalid application audio pipeline."))?;
        let sink = pipeline
            .by_name("pcm")
            .context("Missing audio output.")?
            .downcast::<AppSink>()
            .map_err(|_| anyhow::anyhow!("Invalid audio output."))?;
        // Own the pipeline before adding inputs, so any partial setup error
        // releases every acquired loopback interface through the same Drop path.
        let mut active = Self {
            pipeline,
            sink,
            #[cfg(windows)]
            _leases: Vec::new(),
        };
        let mixer = active
            .pipeline
            .by_name("mix")
            .context("Missing audio mixer.")?;
        for root in roots {
            ensure!(
                !stop.load(Ordering::Acquire),
                "Audio capture was cancelled."
            );
            #[cfg(windows)]
            {
                active._leases.push(windows::lease(*root)?);
            }
            let branch = gst::parse::bin_from_description(
                &format!(
                    "wasapi2src loopback=true loopback-mode=include-process-tree loopback-target-pid={} low-latency=true provide-clock=false ! audioconvert ! audioresample ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! queue max-size-buffers=4 max-size-bytes=0 max-size-time=0 leaky=downstream",
                    root.pid
                ),
                true,
            )?;
            active.pipeline.add(&branch)?;
            branch.link(&mixer)?;
        }
        ensure!(
            !stop.load(Ordering::Acquire),
            "Audio capture was cancelled."
        );
        active
            .pipeline
            .set_state(gst::State::Playing)
            .context("Application audio could not start.")?;
        Ok(active)
    }

    fn check(&self) -> Result<()> {
        if let Some(bus) = self.pipeline.bus() {
            while let Some(message) = bus.pop() {
                match message.view() {
                    gst::MessageView::Error(_) => bail!(
                        "Application audio capture failed. Stop sharing and choose the audio source again."
                    ),
                    gst::MessageView::Eos(_) => bail!("Application audio capture ended."),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        self.pipeline
            .set_state(gst::State::Null)
            .context("Application audio did not stop cleanly.")?;
        let (result, state, _) = self.pipeline.state(gst::ClockTime::from_seconds(3));
        result.context("Application audio did not stop cleanly.")?;
        ensure!(
            state == gst::State::Null,
            "Application audio is still stopping."
        );
        Ok(())
    }
}

// Invoked on the publisher's blocking worker. Only explicit process-tree
// loopbacks are ever opened: enumeration failures cannot fall back to system mix.
pub fn run(
    choice: Choice,
    exclusions: Vec<String>,
    stop: Arc<AtomicBool>,
    snapshot: Arc<Mutex<Metrics>>,
    source: Option<NativeAudioSource>,
    duration: Option<Duration>,
) -> Result<Metrics> {
    #[cfg(not(windows))]
    {
        let _ = (choice, exclusions, stop, snapshot, source, duration);
        bail!("Application audio is currently available on Windows only.");
    }
    #[cfg(windows)]
    {
        ensure!(
            choice != Choice::Off,
            "Choose an application audio mode first."
        );
        ensure!(
            !stop.load(Ordering::Acquire),
            "Audio capture was cancelled."
        );
        let owner = windows::identity(std::process::id())?;
        let mut roots = windows::inventory()?.roots(&choice, &exclusions, owner);
        ensure!(
            roots.len() <= 16,
            "Too many audio applications. Choose one application or exclude some from the desktop mix."
        );
        let mut pipeline = Pipeline::start(&roots, &stop)?;
        let started = Instant::now();
        let mut refresh = Instant::now();
        let mut last_frame = Instant::now();
        let mut counters = Metrics {
            applications: roots.len(),
            ..Default::default()
        };
        let mut meter = Meter::default();
        let mut window = Instant::now();
        let mut window_tones = [0.0_f64; 3];
        while !stop.load(Ordering::Acquire)
            && duration.is_none_or(|limit| started.elapsed() < limit)
        {
            if refresh.elapsed() >= Duration::from_millis(500) {
                let next = windows::inventory()?.roots(&choice, &exclusions, owner);
                ensure!(
                    next.len() <= 16,
                    "Too many audio applications. Choose one application or exclude some from the desktop mix."
                );
                if next != roots {
                    // Fully release old process captures and queued PCM before
                    // opening the new set. Never reuse a capture after PID reuse.
                    pipeline.stop()?;
                    drop(pipeline);
                    if let Some(source) = &source {
                        source.clear_buffer();
                    }
                    pipeline = Pipeline::start(&next, &stop)?;
                    roots = next;
                    counters.applications = roots.len();
                    last_frame = Instant::now();
                }
                refresh = Instant::now();
            }
            pipeline.check()?;
            let Some(sample) = pipeline
                .sink
                .try_pull_sample(gst::ClockTime::from_mseconds(50))
            else {
                ensure!(
                    last_frame.elapsed() < Duration::from_secs(3),
                    "Application audio stopped producing samples."
                );
                continue;
            };
            last_frame = Instant::now();
            if stop.load(Ordering::Acquire) {
                break;
            }
            let buffer = sample.buffer().context("Missing audio samples.")?;
            if let Some(age) = pipeline
                .pipeline
                .current_running_time()
                .zip(buffer.pts())
                .and_then(|(now, pts)| now.checked_sub(pts))
            {
                counters.capture_age_ms = age.nseconds() as f64 / 1_000_000.0;
                counters.capture_age_max_ms =
                    counters.capture_age_max_ms.max(counters.capture_age_ms);
            }
            let bytes = buffer
                .map_readable()
                .context("Could not read audio samples.")?;
            ensure!(
                bytes.len() % 4 == 0 && bytes.len() <= 192_000,
                "Invalid application audio format."
            );
            let samples: Vec<i16> = bytes
                .chunks_exact(2)
                .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
                .collect();
            counters.frames += (samples.len() / 2) as u64;
            counters.peak = samples
                .iter()
                .map(|sample| (*sample as f64).abs() / 32768.0)
                .fold(0.0, f64::max);
            counters.channel_difference_peak = samples
                .chunks_exact(2)
                .map(|frame| ((frame[0] as f64) - (frame[1] as f64)).abs() / 32768.0)
                .fold(counters.channel_difference_peak, f64::max);
            if duration.is_some() {
                meter.push(&samples, &mut window_tones);
                for (all, current) in counters.tones.iter_mut().zip(window_tones) {
                    *all = all.max(current);
                }
                if window.elapsed() >= Duration::from_secs(1) {
                    counters.windows.push(SpectralWindow {
                        second: started.elapsed().as_secs(),
                        tones: window_tones,
                    });
                    window_tones = [0.0; 3];
                    window = Instant::now();
                }
            }
            if let Some(source) = &source {
                let frame = AudioFrame {
                    samples_per_channel: (samples.len() / 2) as u32,
                    data: samples.into(),
                    sample_rate: 48000,
                    num_channels: 2,
                };
                let transport_started = Instant::now();
                tokio::runtime::Handle::current().block_on(async {
                    tokio::time::timeout(Duration::from_millis(250), source.capture_frame(&frame))
                        .await
                        .context("Audio transport stalled.")?
                        .map_err(|_| anyhow::anyhow!("Audio transport could not accept samples."))
                })?;
                counters.transport_wait_ms = transport_started.elapsed().as_secs_f64() * 1000.0;
            }
            if let Ok(mut state) = snapshot.lock() {
                *state = counters.clone();
            }
        }
        pipeline.stop()?;
        drop(pipeline);
        if let Some(source) = &source {
            source.clear_buffer();
        }
        Ok(counters)
    }
}

#[derive(Default)]
pub(crate) struct Meter {
    samples: Vec<f64>,
}
impl Meter {
    pub(crate) fn push(&mut self, stereo: &[i16], levels: &mut [f64; 3]) {
        for frame in stereo.chunks_exact(2) {
            self.samples.push(frame[0] as f64 / 32768.0);
            if self.samples.len() == 4800 {
                for (index, frequency) in [440.0, 880.0, 1320.0].into_iter().enumerate() {
                    let mut real = 0.0;
                    let mut imaginary = 0.0;
                    for (position, sample) in self.samples.iter().enumerate() {
                        let weight = 0.5
                            - 0.5
                                * (std::f64::consts::TAU * position as f64
                                    / self.samples.len() as f64)
                                    .cos();
                        let phase = std::f64::consts::TAU * frequency * position as f64 / 48000.0;
                        real += sample * weight * phase.cos();
                        imaginary += sample * weight * phase.sin();
                    }
                    levels[index] =
                        levels[index].max(4.0 * real.hypot(imaginary) / self.samples.len() as f64);
                }
                self.samples.clear();
            }
        }
    }
}

/// Source selection owns audio policy; callers cannot accidentally capture another app.
pub fn for_source(source: &crate::sources::Source) -> Result<Choice> {
    match source.target {
        crate::sources::Target::Monitor(_) | crate::sources::Target::MonitorWgc(_) => {
            Ok(Choice::Desktop)
        }
        crate::sources::Target::Window { pid, .. } => {
            #[cfg(windows)]
            {
                let inventory = windows::inventory()?;
                let process = inventory
                    .processes
                    .get(&pid)
                    .context("The selected window application ended.")?;
                ensure!(
                    process.identity.created != 0,
                    "Cannot safely identify this window's audio process."
                );
                Ok(Choice::Window(process.identity))
            }
            #[cfg(not(windows))]
            {
                let _ = pid;
                bail!("Application audio requires Windows.")
            }
        }
        crate::sources::Target::Pattern => Ok(Choice::Off),
    }
}
