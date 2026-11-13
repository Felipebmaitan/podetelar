//! Opt-in local flash/click fixtures. Reports media-stage events, never physical
//! screen/speaker synchronization. Ordinary playback does not create an observer.
use anyhow::{Context, Result, ensure};
use eframe::egui::ColorImage;
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::{AppSink, AppSrc};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub struct Events(Mutex<State>);
struct State {
    started: Instant,
    output_windows: Vec<OutputWindow>,
    meter: crate::audio::Meter,
    white: bool,
    last_audio: Option<Instant>,
    video: Vec<Instant>,
    audio: Vec<Instant>,
    capture_ages: Vec<f64>,
    failed: bool,
}
#[derive(serde::Serialize)]
struct OutputWindow {
    second: u64,
    tones: [f64; 3],
    peak: f64,
    channel_difference_peak: f64,
    samples: u64,
}
impl Events {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(Mutex::new(State {
            started: Instant::now(),
            output_windows: Vec::new(),
            meter: crate::audio::Meter::default(),
            white: false,
            last_audio: None,
            video: Vec::new(),
            audio: Vec::new(),
            capture_ages: Vec::new(),
            failed: false,
        })))
    }
    pub fn video(&self, image: &ColorImage) {
        let Some(pixel) = image
            .pixels
            .get(image.size[1] / 2 * image.size[0] + image.size[0] / 2)
        else {
            return;
        };
        let white = pixel.r() > 220 && pixel.g() > 220 && pixel.b() > 220;
        if let Ok(mut state) = self.0.lock() {
            if white && !state.white && state.video.len() < 128 {
                state.video.push(Instant::now());
            }
            state.white = white;
        }
    }
    fn audio(&self, bytes: &[u8], age: Option<f64>) {
        let pcm: Vec<_> = bytes
            .chunks_exact(2)
            .map(|s| i16::from_le_bytes([s[0], s[1]]))
            .collect();
        let peak = pcm.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0) as f64 / 32768.0;
        let difference = pcm
            .chunks_exact(2)
            .map(|s| (s[0] as f64 - s[1] as f64).abs() / 32768.0)
            .fold(0.0_f64, f64::max);
        if let Ok(mut state) = self.0.lock() {
            let now = Instant::now();
            let second = state.started.elapsed().as_secs();
            if second < 121 {
                if state
                    .output_windows
                    .last()
                    .is_none_or(|window| window.second != second)
                {
                    state.output_windows.push(OutputWindow {
                        second,
                        tones: [0.0; 3],
                        peak: 0.0,
                        channel_difference_peak: 0.0,
                        samples: 0,
                    });
                    state.meter = crate::audio::Meter::default();
                }
                let mut tones = state
                    .output_windows
                    .last()
                    .map(|w| w.tones)
                    .unwrap_or_default();
                state.meter.push(&pcm, &mut tones);
                if let Some(window) = state.output_windows.last_mut() {
                    window.tones = tones;
                    window.peak = window.peak.max(peak);
                    window.channel_difference_peak = window.channel_difference_peak.max(difference);
                    window.samples += pcm.len() as u64;
                }
            }
            if peak <= 600.0 / 32768.0 {
                return;
            }
            if state
                .last_audio
                .is_none_or(|last| now.duration_since(last) > Duration::from_millis(300))
                && state.audio.len() < 128
            {
                state.audio.push(now);
                if let Some(age) = age {
                    state.capture_ages.push(age);
                }
            }
            state.last_audio = Some(now);
        }
    }
    pub fn report(&self) -> serde_json::Value {
        let Ok(state) = self.0.lock() else {
            return serde_json::json!({"available":false});
        };
        let millis = |audio: Instant, video: Instant| {
            if audio >= video {
                audio.duration_since(video).as_secs_f64() * 1000.0
            } else {
                -video.duration_since(audio).as_secs_f64() * 1000.0
            }
        };
        // Mutual nearest events inside 400 ms. Nonuniform spacing reduces
        // periodic ambiguity; missing pairs remain explicit and are not evidence
        // that an offset outside the measurement window is acceptable.
        let mut offsets = Vec::new();
        for (vi, &video) in state.video.iter().enumerate() {
            let nearest = state.audio.iter().enumerate().min_by(|a, b| {
                millis(*a.1, video)
                    .abs()
                    .total_cmp(&millis(*b.1, video).abs())
            });
            if let Some((_, &audio)) = nearest {
                let reverse = state.video.iter().enumerate().min_by(|a, b| {
                    millis(audio, *a.1)
                        .abs()
                        .total_cmp(&millis(audio, *b.1).abs())
                });
                let offset = millis(audio, video);
                if reverse.is_some_and(|(index, _)| index == vi) && offset.abs() < 400.0 {
                    offsets.push(offset);
                }
            }
        }
        serde_json::json!({"available":!state.failed,"outputWindows":state.output_windows,"videoEvents":state.video.len(),"audioEvents":state.audio.len(),"pairedEvents":offsets.len(),"unmatchedVideoEvents":state.video.len()-offsets.len(),"unmatchedAudioEvents":state.audio.len()-offsets.len(),"videoMailboxToAudioLoopbackMs":offsets,"loopbackCaptureAgesMs":state.capture_ages,"physicalAvSync":"unavailable","pairingWindowMs":400,"timingIncludes":"loopback capture and callback scheduling; video is mailbox publication, not display presentation"})
    }
}

pub struct Observer {
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Observer {
    pub fn start(events: Arc<Events>) -> Result<Self> {
        let pipeline = gst::parse::launch(&format!("wasapi2src loopback=true loopback-mode=include-process-tree loopback-target-pid={} low-latency=true provide-clock=false ! audioconvert ! audioresample ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! appsink name=events max-buffers=4 drop=true sync=false",std::process::id()))?
            .downcast::<gst::Pipeline>().map_err(|_|anyhow::anyhow!("Invalid A/V observer."))?;
        let pipeline = crate::playback::AudioPipeline(pipeline);
        let sink = pipeline
            .0
            .by_name("events")
            .context("Missing A/V observer output.")?
            .downcast::<AppSink>()
            .map_err(|_| anyhow::anyhow!("Invalid A/V observer output."))?;
        pipeline.0.set_state(gst::State::Playing)?;
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let worker = std::thread::Builder::new()
            .name("av-fixture-loopback".into())
            .spawn(move || {
                while !cancelled.load(Ordering::Acquire) {
                    if pipeline
                        .0
                        .bus()
                        .is_some_and(|bus| bus.pop_filtered(&[gst::MessageType::Error]).is_some())
                    {
                        if let Ok(mut state) = events.0.lock() {
                            state.failed = true;
                        }
                        break;
                    }
                    if let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_mseconds(20))
                        && let Some(buffer) = sample.buffer()
                        && let Ok(map) = buffer.map_readable()
                    {
                        let age = pipeline
                            .0
                            .current_running_time()
                            .zip(buffer.pts())
                            .and_then(|(now, pts)| now.checked_sub(pts))
                            .map(|v| v.nseconds() as f64 / 1e6);
                        events.audio(&map, age);
                    }
                }
            })?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}
impl Drop for Observer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// An explicit generated source in its own GUI window. Tests run a copy named
/// Game.exe so the publisher captures this source alone, respecting exclusion.
pub fn source() -> Result<()> {
    let pipeline=gst::parse::launch("appsrc name=video is-live=true format=time ! video/x-raw,format=BGRA,width=640,height=360,framerate=60/1 ! queue max-size-buffers=3 max-size-bytes=0 max-size-time=0 ! videoconvert ! d3d11videosink name=screen sync=true appsrc name=audio is-live=true format=time ! audio/x-raw,format=S16LE,rate=48000,channels=2,layout=interleaved ! queue ! wasapi2sink low-latency=true buffer-time=20000 sync=true")?
        .downcast::<gst::Pipeline>().map_err(|_|anyhow::anyhow!("Invalid A/V source."))?;
    let pipeline = crate::playback::AudioPipeline(pipeline);
    let input = |name| {
        pipeline
            .0
            .by_name(name)
            .context("Missing A/V source input.")?
            .downcast::<AppSrc>()
            .map_err(|_| anyhow::anyhow!("Invalid A/V source input."))
    };
    let video = input("video")?;
    let audio = input("audio")?;
    // The same PTS schedules both branches. Unequal pulse intervals expose
    // missed/mispaired events instead of concealing a periodic whole-cycle slip.
    let mut pulses = Vec::new();
    let mut event = 2_000_000_000_u64;
    for interval in [1_100_000_000, 1_700_000_000, 2_300_000_000, 1_400_000_000]
        .into_iter()
        .cycle()
        .take(60)
    {
        pulses.push(event);
        event += interval;
    }
    pipeline.0.set_state(gst::State::Playing)?;
    let started = Instant::now();
    for tick in 0..18_000_u64 {
        let pts = tick * 1_000_000_000 / 300;
        std::thread::sleep(
            (started + Duration::from_nanos(pts)).saturating_duration_since(Instant::now()),
        );
        ensure!(
            pipeline
                .0
                .bus()
                .is_none_or(|bus| bus.pop_filtered(&[gst::MessageType::Error]).is_none()),
            "A/V source output failed."
        );
        if tick % 5 == 0 {
            let white = pulses
                .iter()
                .any(|&event| pts >= event && pts < event + 100_000_000);
            let mut pixels = vec![if white { 255 } else { 0 }; 640 * 360 * 4];
            for pixel in pixels.chunks_exact_mut(4) {
                pixel[3] = 255;
            }
            let mut buffer = gst::Buffer::from_mut_slice(pixels);
            let data = buffer.get_mut().context("A/V source buffer is shared.")?;
            data.set_pts(gst::ClockTime::from_nseconds(pts));
            data.set_duration(gst::ClockTime::from_nseconds(1_000_000_000 / 60));
            video.push_buffer(buffer)?;
        }
        if tick % 3 == 0 {
            let samples = (0..480_u64)
                .flat_map(|sample| {
                    let time = pts + sample * 1_000_000_000 / 48000;
                    let value = pulses
                        .iter()
                        .find_map(|&event| {
                            (time >= event && time < event + 20_000_000).then(|| {
                                (3000.0
                                    * (std::f64::consts::TAU * 1000.0 * (time - event) as f64
                                        / 1e9)
                                        .sin()) as i16
                            })
                        })
                        .unwrap_or(0);
                    [value, value]
                })
                .collect();
            let mut buffer = crate::playback::audio_buffer(samples);
            let data = buffer.get_mut().context("A/V source buffer is shared.")?;
            data.set_pts(gst::ClockTime::from_nseconds(pts));
            data.set_duration(gst::ClockTime::from_mseconds(10));
            audio.push_buffer(buffer)?;
        }
    }
    Ok(())
}
