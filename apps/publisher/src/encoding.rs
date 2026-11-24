use crate::preview::{self, Frame, Latest};
pub use crate::quality::{Codec, Mode};
use crate::quality::{EncoderBudget, RateControl, ReinitializingBudget};
use crate::sources::{self, Source, Target};
use anyhow::{Context, Result, bail, ensure};
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSink;
use livekit::webrtc::{
    video_frame::{EncodedFrameType, EncodedVideoCodec, EncodedVideoFrame},
    video_source::{VideoResolution, native::NativeVideoSource},
};
use serde::Serialize;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize)]
pub struct Encoder {
    pub factory: String,
    pub label: String,
    pub codec: Codec,
}

pub fn encoders() -> Vec<Encoder> {
    [
        ("vtenc_h265_hw", "Apple VideoToolbox HEVC", Codec::H265),
        ("vtenc_h264_hw", "Apple VideoToolbox H.264", Codec::H264),
        ("nvd3d11av1enc", "NVIDIA NVENC AV1", Codec::Av1),
        ("amfav1enc", "AMD AMF AV1", Codec::Av1),
        ("nvd3d11h265enc", "NVIDIA NVENC HEVC", Codec::H265),
        ("amfh265enc", "AMD AMF HEVC", Codec::H265),
        ("nvd3d11h264enc", "NVIDIA NVENC H.264", Codec::H264),
        ("amfh264enc", "AMD AMF H.264", Codec::H264),
    ]
    .into_iter()
    .filter(|(factory, _, _)| gst::ElementFactory::find(factory).is_some())
    .map(|(factory, label, codec)| Encoder {
        factory: factory.into(),
        label: label.into(),
        codec,
    })
    .collect()
}

#[derive(Default, Clone, Serialize)]
pub struct Metrics {
    pub elapsed_seconds: f64,
    pub frames: u64,
    pub accepted: u64,
    pub bytes: u64,
    pub fps: f64,
    pub encoder_bitrate: u64,
    pub preview_frames: u64,
    pub requested_bitrate: u64,
    pub encoded_bps: u64,
    pub encoder_bitrate_changes: u64,
    pub congestion_bps: Option<u64>,
    pub capture_queue_frames: Option<u32>,
    pub encoder_queue_frames: Option<u32>,
}

/// A bounded local view of the existing camera capture. The presentation lease
/// expires even if the window stops drawing frames without a visibility event.
pub struct CameraFeed {
    sink: AppSink,
    requested_at: Arc<Mutex<Option<Instant>>>,
}

impl CameraFeed {
    fn new(pipeline: &gst::Pipeline) -> Result<Self> {
        let sink = pipeline
            .by_name("camera_view")
            .and_downcast::<AppSink>()
            .context("Missing local camera sink.")?;
        let requested_at = Arc::new(Mutex::new(None::<Instant>));
        let requested = requested_at.clone();
        pipeline
            .by_name("camera_view_gate")
            .and_then(|queue| queue.static_pad("sink"))
            .context("Missing local camera gate.")?
            .add_probe(gst::PadProbeType::BUFFER, move |_, _| {
                if requested.lock().is_ok_and(|time| {
                    time.is_some_and(|time| time.elapsed() < Duration::from_millis(200))
                }) {
                    gst::PadProbeReturn::Ok
                } else {
                    gst::PadProbeReturn::Drop
                }
            });
        Ok(Self { sink, requested_at })
    }

    pub fn next_sample(&self) -> Option<gst::Sample> {
        *self.requested_at.lock().ok()? = Some(Instant::now());
        self.sink.try_pull_sample(gst::ClockTime::ZERO)
    }
}

// Owns the native pipeline. Every return/error path transitions it to NULL.
pub struct Encoding {
    #[cfg(target_os = "macos")]
    _capture: Option<crate::macos::Capture>,
    pipeline: gst::Pipeline,
    sink: AppSink,
    encoder: gst::Element,
    pub mode: Mode,
    codec: Codec,
    first: Option<gst::Sample>,
    epoch_us: i64,
    capture: Source,
    preview_sink: AppSink,
    previews: Latest,
    pub camera_feed: Option<Arc<CameraFeed>>,
    rate: RateControl,
}

impl Drop for Encoding {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

impl Encoding {
    pub fn rate(&self) -> RateControl {
        self.rate.clone()
    }
    pub fn previews(&self) -> Latest {
        self.previews.clone()
    }
    pub fn codec(&self) -> Codec {
        self.codec
    }
    pub fn start(
        source: &Source,
        encoder: &Encoder,
        mode: Mode,
        stop: &AtomicBool,
    ) -> Result<Self> {
        Self::start_controlled(source, encoder, mode, stop, RateControl::new(mode.bitrate))
    }
    pub fn start_controlled(
        source: &Source,
        encoder: &Encoder,
        mode: Mode,
        stop: &AtomicBool,
        rate: RateControl,
    ) -> Result<Self> {
        Self::start_with_preview(source, encoder, mode, stop, rate, preview::Capture::new())
    }
    pub fn start_with_preview(
        source: &Source,
        encoder: &Encoder,
        mode: Mode,
        stop: &AtomicBool,
        rate: RateControl,
        preview: preview::Capture,
    ) -> Result<Self> {
        ensure!(!stop.load(Ordering::Acquire), "Capture was cancelled.");
        ensure!(
            encoder.supports(mode),
            "This source mode is unavailable on the selected hardware encoder."
        );
        ensure!(
            sources::validate(source),
            "That capture source is no longer available. Refresh the list."
        );
        ensure!(
            encoders()
                .iter()
                .any(|candidate| candidate.factory == encoder.factory
                    && candidate.codec == encoder.codec),
            "The selected hardware encoder is unavailable."
        );
        let (media_type, profile, framing) = match encoder.codec {
            Codec::Av1 => (
                "video/x-av1",
                "main",
                "av1parse ! video/x-av1,stream-format=obu-stream,alignment=tu",
            ),
            Codec::H265 => (
                "video/x-h265",
                "main",
                "h265parse config-interval=-1 ! video/x-h265,stream-format=byte-stream,alignment=au",
            ),
            Codec::H264 => (
                "video/x-h264",
                if cfg!(target_os = "macos") {
                    "baseline"
                } else {
                    "constrained-baseline"
                },
                "h264parse config-interval=-1 ! video/x-h264,stream-format=byte-stream,alignment=au",
            ),
        };
        // Only allowlisted factory names and bounded numeric settings enter this pipeline.
        // WGC can stop delivering buffers while a window is unchanged. Retain the
        // latest GPU texture and clock its output so a late viewer can request a
        // fresh keyframe even on a static scene. ANY caps preserve D3D11 memory.
        #[cfg(not(target_os = "macos"))]
        let cadence = if matches!(source.target, Target::Window { .. } | Target::MonitorWgc(_)) {
            "imagefreeze is-live=true allow-replace=true ! "
        } else {
            ""
        };
        #[cfg(not(target_os = "macos"))]
        let camera_decode = match &source.target {
            #[cfg(windows)]
            Target::Camera(input)
                if input
                    .caps
                    .structure(0)
                    .is_some_and(|s| s.name() == "image/jpeg") =>
            {
                "jpegdec ! "
            }
            _ => "",
        };
        #[cfg(not(target_os = "macos"))]
        let upload = if source.is_camera() {
            "d3d11upload ! "
        } else {
            ""
        };
        #[cfg(not(target_os = "macos"))]
        let camera_view = if source.is_camera() {
            " raw. ! queue name=camera_view_gate max-size-buffers=1 max-size-bytes=0 max-size-time=0 leaky=downstream ! d3d11convert add-borders=true ! video/x-raw(memory:D3D11Memory),format=RGBA,width=640,height=360 ! d3d11download ! video/x-raw,format=RGBA ! appsink name=camera_view max-buffers=1 drop=true sync=false async=false wait-on-eos=false enable-last-sample=false"
        } else {
            ""
        };
        #[cfg(not(target_os = "macos"))]
        let processing = format!(
            "queue name=entry max-size-buffers=2 max-size-bytes=0 max-size-time=0 leaky=downstream ! {camera_decode}{cadence}videorate drop-only=true max-rate={} ! {upload}d3d11convert ! video/x-raw(memory:D3D11Memory),format=NV12,width={},height={},framerate={}/1 ! clocksync ! tee name=raw raw. ! queue name=encoder_queue max-size-buffers=2 max-size-bytes=0 max-size-time=0 ! {} name=hardware bitrate={} gop-size={} ! {media_type},profile={profile} ! {framing} ! appsink name=encoded max-buffers=2 sync=false raw. ! queue name=preview_gate max-size-buffers=1 max-size-bytes=0 max-size-time=0 leaky=downstream ! d3d11convert add-borders=true ! video/x-raw(memory:D3D11Memory),format=NV12,width=960,height=540 ! d3d11download ! video/x-raw,format=NV12 ! videoconvert ! video/x-raw,format=I420 ! jpegenc quality=85 ! appsink name=preview max-buffers=1 drop=true sync=false async=false wait-on-eos=false{camera_view}",
            mode.fps,
            mode.width,
            mode.height,
            mode.fps,
            encoder.factory,
            rate.effective(None) / 1000,
            mode.fps
        );
        #[cfg(target_os = "macos")]
        let processing = format!(
            "queue name=entry max-size-buffers=2 max-size-bytes=0 max-size-time=0 leaky=downstream ! imagefreeze is-live=true allow-replace=true ! videorate drop-only=true max-rate={} ! videoconvert ! videoscale add-borders=true ! video/x-raw,format=NV12,width={},height={},framerate={}/1 ! tee name=raw raw. ! queue name=encoder_queue max-size-buffers=2 max-size-bytes=0 max-size-time=0 ! {} name=hardware bitrate={} max-keyframe-interval={} realtime=true allow-frame-reordering=false ! {media_type},profile={profile} ! {framing} ! appsink name=encoded max-buffers=2 sync=false raw. ! queue name=preview_gate max-size-buffers=1 max-size-bytes=0 max-size-time=0 leaky=downstream ! videoscale add-borders=true ! video/x-raw,width=960,height=540 ! videoconvert ! video/x-raw,format=I420 ! jpegenc quality=85 ! appsink name=preview max-buffers=1 drop=true sync=false async=false wait-on-eos=false",
            mode.fps,
            mode.width,
            mode.height,
            mode.fps,
            encoder.factory,
            rate.effective(None) / 1000,
            mode.fps
        );
        let pipeline = gst::parse::launch(&processing)
            .context("Could not create the GPU encoding pipeline.")?
            .downcast::<gst::Pipeline>()
            .map_err(|_| anyhow::anyhow!("Invalid encoding pipeline."))?;
        #[cfg(target_os = "macos")]
        let input = crate::macos::input(source)?;
        #[cfg(not(target_os = "macos"))]
        let input = match source.target {
            #[cfg(windows)]
            Target::Camera(ref camera) => camera
                .device
                .create_element(None)
                .context("Could not open the selected camera.")?,
            Target::Pattern => gst::ElementFactory::make("d3d11testsrc")
                .property("is-live", true)
                .build()
                .context("GPU test source is unavailable.")?,
            Target::Monitor(handle) => gst::ElementFactory::make("d3d11screencapturesrc")
                .property("monitor-handle", handle)
                .property("show-cursor", true)
                .build()
                .context("Monitor capture is unavailable.")?,
            Target::MonitorWgc(handle) => gst::ElementFactory::make("d3d11screencapturesrc")
                .property_from_str("capture-api", "wgc")
                .property("monitor-handle", handle)
                .property("show-cursor", true)
                .build()
                .context("Windows Graphics Capture is unavailable.")?,
            Target::Window { handle, .. } => gst::ElementFactory::make("d3d11screencapturesrc")
                .property_from_str("capture-api", "wgc")
                .property("window-handle", handle)
                .property("show-cursor", true)
                .build()
                .context("Windows Graphics Capture is unavailable.")?,
        };
        pipeline.add(&input)?;
        let entry = pipeline
            .by_name("entry")
            .context("Missing capture queue.")?;
        let link = || {
            #[cfg(windows)]
            if let Target::Camera(camera) = &source.target {
                return input.link_filtered(&entry, &camera.caps);
            }
            if matches!(source.target, Target::Pattern) {
                // A high-resolution fixture must originate at that size, rather than
                // accidentally qualifying an upscaled default test texture.
                let caps = gst::Caps::builder("video/x-raw");
                #[cfg(not(target_os = "macos"))]
                let caps = caps.features(["memory:D3D11Memory"]);
                input.link_filtered(
                    &entry,
                    &caps
                        .field("width", source.info.width as i32)
                        .field("height", source.info.height as i32)
                        .field("framerate", gst::Fraction::new(mode.fps as i32, 1))
                        .build(),
                )
            } else {
                input.link(&entry)
            }
        };
        link().context("Capture source could not connect to GPU conversion.")?;
        let enabled = rate.diagnostic_enabled.clone();
        let overruns = rate.capture_overruns.clone();
        entry.connect("overrun", false, move |_| {
            if enabled.load(Ordering::Relaxed) {
                overruns.fetch_add(1, Ordering::Relaxed);
            }
            None
        });
        let hardware = pipeline
            .by_name("hardware")
            .context("Missing hardware encoder.")?;
        if encoder.factory.starts_with("vtenc_") {
            // The _hw factories require VideoToolbox hardware; no software fallback.
        } else if encoder.factory.starts_with("nv") {
            hardware.set_property_from_str("preset", "p4");
            hardware.set_property_from_str("tune", "ultra-low-latency");
            hardware.set_property_from_str("rc-mode", "cbr");
            hardware.set_property("bframes", 0_u32);
            hardware.set_property("zerolatency", true);
        } else {
            hardware.set_property_from_str(
                "usage",
                if encoder.codec == Codec::Av1 {
                    "low-latency"
                } else {
                    "ultra-low-latency"
                },
            );
            hardware.set_property_from_str("rate-control", "cbr");
        }
        for (pad, count) in [
            (input.static_pad("src"), rate.capture_frames.clone()),
            (hardware.static_pad("sink"), rate.encoder_inputs.clone()),
        ] {
            let enabled = rate.diagnostic_enabled.clone();
            pad.context("Missing diagnostic pad.")?.add_probe(
                gst::PadProbeType::BUFFER,
                move |_, _| {
                    if enabled.load(Ordering::Relaxed) {
                        count.fetch_add(1, Ordering::Relaxed);
                    }
                    gst::PadProbeReturn::Ok
                },
            );
        }
        let sink = pipeline
            .by_name("encoded")
            .context("Missing encoded frame sink.")?
            .downcast::<AppSink>()
            .map_err(|_| anyhow::anyhow!("Invalid frame sink."))?;
        let gate = preview.gate;
        pipeline
            .by_name("preview_gate")
            .and_then(|element| element.static_pad("sink"))
            .context("Missing preview gate.")?
            .add_probe(gst::PadProbeType::BUFFER, move |_, _| {
                if gate.take() {
                    gst::PadProbeReturn::Ok
                } else {
                    gst::PadProbeReturn::Drop
                }
            });
        let preview_sink = pipeline
            .by_name("preview")
            .context("Missing preview sink.")?
            .downcast::<AppSink>()
            .map_err(|_| anyhow::anyhow!("Invalid preview sink."))?;
        let camera_feed = source
            .is_camera()
            .then(|| CameraFeed::new(&pipeline).map(Arc::new))
            .transpose()?;
        let epoch_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_micros()
            .min(i64::MAX as u128) as i64;
        #[cfg(target_os = "macos")]
        let capture = crate::macos::video(&pipeline, source, mode.width, mode.height, mode.fps)?;
        let mut encoding = Self {
            #[cfg(target_os = "macos")]
            _capture: capture,
            pipeline,
            sink,
            encoder: hardware,
            mode,
            codec: encoder.codec,
            first: None,
            epoch_us,
            capture: source.clone(),
            preview_sink,
            previews: preview.latest,
            camera_feed,
            rate,
        };
        ensure!(!stop.load(Ordering::Acquire), "Capture was cancelled.");
        encoding
            .pipeline
            .set_state(gst::State::Playing)
            .context("Hardware capture could not start.")?;
        let started = Instant::now();
        while !stop.load(Ordering::Acquire) && started.elapsed() < Duration::from_secs(8) {
            encoding.check_bus()?;
            if let Some(sample) = encoding
                .sink
                .try_pull_sample(gst::ClockTime::from_mseconds(50))
            {
                encoding.validate_dimensions()?;
                encoding.first = Some(sample);
                return Ok(encoding);
            }
        }
        bail!("Capture was cancelled or the hardware encoder did not produce a frame.")
    }

    fn check_bus(&self) -> Result<()> {
        if let Some(bus) = self.pipeline.bus() {
            while let Some(message) = bus.pop() {
                match message.view() {
                    gst::MessageView::Error(_) => bail!(
                        "The capture source or GPU encoder stopped. Refresh sources and try again."
                    ),
                    gst::MessageView::Eos(_) => bail!("The capture source ended."),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn validate_dimensions(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        if !matches!(self.capture.target, Target::Pattern) {
            let actual = crate::macos::enumerate()
                .into_iter()
                .find(|source| source.info.id == self.capture.info.id)
                .context("The capture source closed.")?;
            ensure!(
                actual.info.width >= self.mode.width && actual.info.height >= self.mode.height,
                "Source is smaller than the selected stream resolution. Choose a smaller resolution and share again."
            );
        }

        let caps = self
            .pipeline
            .by_name("entry")
            .and_then(|entry| entry.static_pad("sink"))
            .and_then(|pad| pad.current_caps())
            .context("Capture dimensions are unavailable.")?;
        let caps = caps
            .structure(0)
            .context("Capture dimensions are unavailable.")?;
        let width = caps.get::<i32>("width").unwrap_or(0);
        let height = caps.get::<i32>("height").unwrap_or(0);
        ensure!(
            width >= self.mode.width as i32 && height >= self.mode.height as i32,
            "Source is smaller than the selected stream resolution. Choose a smaller resolution and share again."
        );
        Ok(())
    }

    fn keyframe(&self, count: u32) {
        self.sink.send_event(
            gstreamer_video::UpstreamForceKeyUnitEvent::builder()
                .all_headers(true)
                .count(count)
                .build(),
        );
    }

    pub fn run(
        mut self,
        source: Option<NativeVideoSource>,
        stop: Arc<AtomicBool>,
        metrics: Arc<Mutex<Metrics>>,
        frame_limit: Option<u64>,
    ) -> Result<Metrics> {
        let started = Instant::now();
        let mut last_frame = Instant::now();
        let mut last_report = Instant::now();
        let mut counters = Metrics {
            encoder_bitrate: self.encoder.property::<u32>("bitrate") as u64 * 1000,
            ..Default::default()
        };
        let mut keyframe_count = 0;
        let mut feedback = None;
        let mut budget = EncoderBudget::new(counters.encoder_bitrate);
        let reinitializing = self
            .encoder
            .factory()
            .is_some_and(|f| f.name().starts_with("amf"));
        let mut amf_budget =
            ReinitializingBudget::new(counters.encoder_bitrate, self.rate.requested());
        let mut first_feedback = false;
        let mut last_rate_update = Instant::now();
        let mut reported_bytes = 0;
        let mut waiting_for_key = true;
        let mut last_key_request = Instant::now() - Duration::from_secs(1);
        while !stop.load(Ordering::Acquire) {
            self.check_bus()?;
            if let Some(sample) = self.preview_sink.try_pull_sample(gst::ClockTime::ZERO)
                && let Some(buffer) = sample.buffer()
                && let Ok(bytes) = buffer.map_readable()
                && bytes.len() <= preview::MAX_BYTES
                && let Ok(mut latest) = self.previews.lock()
            {
                counters.preview_frames += 1;
                *latest = Some(Frame {
                    sequence: latest
                        .as_ref()
                        .map_or(1, |frame| frame.sequence.saturating_add(1)),
                    bytes: bytes.to_vec(),
                });
            }
            if let Some(source) = &source {
                if source.take_keyframe_request()
                    || (waiting_for_key && last_key_request.elapsed() >= Duration::from_millis(500))
                {
                    keyframe_count += 1;
                    self.keyframe(keyframe_count);
                    last_key_request = Instant::now();
                }
                if let Some(target) = source.take_rate_control_request() {
                    first_feedback = feedback.is_none();
                    feedback = Some(target.target_bitrate_bps);
                }
            }
            counters.congestion_bps = feedback;
            let requested = self.rate.requested();
            let user_changed =
                counters.requested_bitrate != 0 && requested != counters.requested_bitrate;
            if first_feedback
                || requested != counters.requested_bitrate
                || last_rate_update.elapsed() >= Duration::from_secs(1)
            {
                // AMF can internally reconfigure on property changes. Coalesce
                // periodic BWE updates; user edits and first feedback apply now.
                let effective = if reinitializing {
                    amf_budget.update(&self.rate, feedback, started.elapsed())
                } else {
                    budget.update(&self.rate, feedback)
                } / 1000
                    * 1000;
                if effective != counters.encoder_bitrate {
                    self.encoder
                        .set_property("bitrate", (effective / 1000) as u32);
                    counters.encoder_bitrate = effective;
                    counters.encoder_bitrate_changes += 1;
                }
                last_rate_update = Instant::now();
                first_feedback = false;
            }
            counters.requested_bitrate = requested;
            if user_changed {
                // A hardware reconfiguration can change codec headers. Resume
                // on a fresh self-contained frame instead of feeding old queued
                // reference frames across the live edit to the decoder.
                keyframe_count += 1;
                self.keyframe(keyframe_count);
                waiting_for_key = true;
                last_key_request = Instant::now();
            }
            let sample = self
                .first
                .take()
                .or_else(|| self.sink.try_pull_sample(gst::ClockTime::from_mseconds(50)));
            let Some(sample) = sample else {
                ensure!(
                    last_frame.elapsed() < Duration::from_secs(8),
                    "Capture stopped producing frames."
                );
                continue;
            };
            last_frame = Instant::now();
            let buffer = sample
                .buffer()
                .context("Encoder returned an empty buffer.")?;
            let payload = buffer
                .map_readable()
                .context("Could not read encoded GPU output.")?;
            let key = !buffer.flags().contains(gst::BufferFlags::DELTA_UNIT);
            counters.frames += 1;
            counters.bytes += payload.len() as u64;
            if let Some(source) = &source
                && (key || !waiting_for_key)
            {
                let timestamp_us = self.epoch_us.saturating_add(
                    buffer
                        .pts()
                        .or(buffer.dts())
                        .map(|pts| pts.useconds())
                        .unwrap_or_else(|| started.elapsed().as_micros() as u64)
                        .min(i64::MAX as u64) as i64,
                );
                let accepted = source.capture_encoded_frame(&EncodedVideoFrame {
                    codec: match self.codec {
                        Codec::Av1 => EncodedVideoCodec::AV1,
                        Codec::H265 => EncodedVideoCodec::H265,
                        Codec::H264 => EncodedVideoCodec::H264,
                    },
                    payload: &payload,
                    timestamp_us,
                    frame_type: if key {
                        EncodedFrameType::Key
                    } else {
                        EncodedFrameType::Delta
                    },
                    resolution: VideoResolution {
                        width: self.mode.width,
                        height: self.mode.height,
                    },
                    frame_metadata: None,
                });
                waiting_for_key = !accepted;
                if accepted {
                    counters.accepted += 1;
                }
            }
            counters.elapsed_seconds = started.elapsed().as_secs_f64();
            counters.fps = counters.frames as f64 / started.elapsed().as_secs_f64().max(0.001);
            if last_report.elapsed() >= Duration::from_millis(500) {
                ensure!(
                    sources::validate(&self.capture),
                    "The capture source closed or disconnected."
                );
                self.validate_dimensions()?;
                counters.encoded_bps = ((counters.bytes - reported_bytes) as f64 * 8.0
                    / last_report.elapsed().as_secs_f64())
                    as u64;
                reported_bytes = counters.bytes;
                if self.rate.diagnostic_enabled.load(Ordering::Relaxed) {
                    counters.capture_queue_frames = self
                        .pipeline
                        .by_name("entry")
                        .map(|q| q.property::<u32>("current-level-buffers"));
                    counters.encoder_queue_frames = self
                        .pipeline
                        .by_name("encoder_queue")
                        .map(|q| q.property::<u32>("current-level-buffers"));
                } else {
                    counters.capture_queue_frames = None;
                    counters.encoder_queue_frames = None;
                }
                if let Ok(mut snapshot) = metrics.lock() {
                    *snapshot = counters.clone();
                }
                last_report = Instant::now();
            }
            if frame_limit.is_some_and(|limit| counters.frames >= limit) {
                break;
            }
        }
        if let Ok(mut snapshot) = metrics.lock() {
            *snapshot = counters.clone();
        }
        self.pipeline
            .set_state(gst::State::Null)
            .context("Native capture did not stop cleanly.")?;
        let (result, state, _) = self.pipeline.state(gst::ClockTime::from_seconds(3));
        result.context("Native capture did not stop cleanly.")?;
        ensure!(
            state == gst::State::Null,
            "Native capture is still stopping."
        );
        Ok(counters)
    }
}

impl Encoder {
    pub fn supports(&self, mode: Mode) -> bool {
        let Some(factory) = gst::ElementFactory::find(&self.factory) else {
            return false;
        };
        let caps = gst::Caps::builder("video/x-raw");
        #[cfg(not(target_os = "macos"))]
        let caps = caps.features(["memory:D3D11Memory"]);
        let caps = caps
            .field("format", "NV12")
            .field("width", mode.width as i32)
            .field("height", mode.height as i32)
            .field("framerate", gst::Fraction::new(mode.fps as i32, 1))
            .build();
        factory.static_pad_templates().iter().any(|template| {
            template.direction() == gst::PadDirection::Sink && template.caps().can_intersect(&caps)
        })
    }
}

#[cfg(test)]
mod camera_view_tests {
    use super::*;

    #[test]
    fn camera_view_stops_receiving_without_a_visible_consumer() -> Result<()> {
        gst::init()?;
        let pipeline = gst::parse::launch(
            "videotestsrc is-live=true ! video/x-raw,format=RGBA,width=64,height=36,framerate=30/1 ! queue name=camera_view_gate max-size-buffers=1 leaky=downstream ! appsink name=camera_view max-buffers=1 drop=true sync=false async=false",
        )?.downcast::<gst::Pipeline>().expect("camera view pipeline");
        let feed = CameraFeed::new(&pipeline)?;
        pipeline.set_state(gst::State::Playing)?;
        let result = (|| -> Result<()> {
            let timeout = gst::ClockTime::from_mseconds(150);
            ensure!(
                feed.sink.try_pull_sample(timeout).is_none(),
                "Hidden camera view received a frame"
            );
            feed.next_sample();
            let sample = feed
                .sink
                .try_pull_sample(timeout)
                .context("First camera view frame")?;
            assert_eq!(
                crate::video_image::camera_image(&sample).unwrap().size,
                [64, 36]
            );
            // A closed view can leave one buffered frame, but it must stop
            // receiving new frames after the presentation lease expires.
            std::thread::sleep(Duration::from_millis(350));
            feed.sink.try_pull_sample(gst::ClockTime::ZERO);
            ensure!(
                feed.sink.try_pull_sample(timeout).is_none(),
                "Camera view kept receiving after the lease expired"
            );
            feed.next_sample();
            ensure!(
                feed.sink.try_pull_sample(timeout).is_some(),
                "Camera view did not resume"
            );
            Ok(())
        })();
        pipeline.set_state(gst::State::Null)?;
        result
    }
}
