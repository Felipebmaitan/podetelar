use crate::preview::{self, Frame, Gate, Latest};
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
    pub frames: u64,
    pub accepted: u64,
    pub bytes: u64,
    pub fps: f64,
    pub encoder_bitrate: u64,
    pub preview_frames: u64,
    pub requested_bitrate: u64,
    pub encoded_bps: u64,
    pub encoder_bitrate_changes: u64,
}

// Owns the native pipeline. Every return/error path transitions it to NULL.
pub struct Encoding {
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
                "constrained-baseline",
                "h264parse config-interval=-1 ! video/x-h264,stream-format=byte-stream,alignment=au",
            ),
        };
        // Only allowlisted factory names and bounded numeric settings enter this pipeline.
        // WGC can stop delivering buffers while a window is unchanged. Retain the
        // latest GPU texture and clock its output so a late viewer can request a
        // fresh keyframe even on a static scene. ANY caps preserve D3D11 memory.
        let cadence = if matches!(source.target, Target::Window { .. } | Target::MonitorWgc(_)) {
            "imagefreeze is-live=true allow-replace=true ! "
        } else {
            ""
        };
        let processing = format!(
            "queue name=entry max-size-buffers=2 max-size-bytes=0 max-size-time=0 leaky=downstream ! {cadence}videorate drop-only=true max-rate={} ! d3d11convert ! video/x-raw(memory:D3D11Memory),format=NV12,width={},height={},framerate={}/1 ! clocksync ! tee name=raw raw. ! queue max-size-buffers=2 max-size-bytes=0 max-size-time=0 ! {} name=hardware bitrate={} gop-size={} ! {media_type},profile={profile} ! {framing} ! appsink name=encoded max-buffers=2 sync=false raw. ! queue name=preview_gate max-size-buffers=1 max-size-bytes=0 max-size-time=0 leaky=downstream ! d3d11convert add-borders=true ! video/x-raw(memory:D3D11Memory),format=NV12,width=960,height=540 ! d3d11download ! video/x-raw,format=NV12 ! videoconvert ! video/x-raw,format=I420 ! jpegenc quality=85 ! appsink name=preview max-buffers=1 drop=true sync=false async=false wait-on-eos=false",
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
        let input = match source.target {
            Target::Pattern => gst::ElementFactory::make("d3d11testsrc")
                .property("is-live", true)
                .property_from_str(
                    "pattern",
                    if std::env::var("FREEDOMSTREAM_FIXTURE_NOISE").as_deref() == Ok("1") {
                        "snow"
                    } else {
                        "smpte"
                    },
                )
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
        if matches!(source.target, Target::Pattern) {
            // A high-resolution fixture must originate at that size, rather than
            // accidentally qualifying an upscaled default test texture.
            input.link_filtered(
                &entry,
                &gst::Caps::builder("video/x-raw")
                    .features(["memory:D3D11Memory"])
                    .field("width", source.info.width as i32)
                    .field("height", source.info.height as i32)
                    .field("framerate", gst::Fraction::new(mode.fps as i32, 1))
                    .build(),
            )
        } else {
            input.link(&entry)
        }
        .context("Capture source could not connect to GPU conversion.")?;
        let hardware = pipeline
            .by_name("hardware")
            .context("Missing hardware encoder.")?;
        if encoder.factory.starts_with("nv") {
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
        let sink = pipeline
            .by_name("encoded")
            .context("Missing encoded frame sink.")?
            .downcast::<AppSink>()
            .map_err(|_| anyhow::anyhow!("Invalid frame sink."))?;
        let gate = Gate::new();
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
        let epoch_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_micros()
            .min(i64::MAX as u128) as i64;
        let mut encoding = Self {
            pipeline,
            sink,
            encoder: hardware,
            mode,
            codec: encoder.codec,
            first: None,
            epoch_us,
            capture: source.clone(),
            preview_sink,
            previews: Arc::new(Mutex::new(None)),
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
                    sequence: counters.preview_frames,
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
        let caps = gst::Caps::builder("video/x-raw")
            .features(["memory:D3D11Memory"])
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
