use crate::sources::{self, Source, Target};
use anyhow::{Context, Result, bail, ensure};
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSink;
use livekit::webrtc::{
    video_frame::{EncodedFrameType, EncodedVideoCodec, EncodedVideoFrame},
    video_source::{VideoResolution, native::NativeVideoSource},
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    H265,
    H264,
}

impl Codec {
    pub fn mime(self) -> &'static str {
        match self {
            Self::H265 => "H.265 / HEVC",
            Self::H264 => "H.264 compatibility",
        }
    }
    pub fn encoded(self) -> EncodedVideoCodec {
        match self {
            Self::H265 => EncodedVideoCodec::H265,
            Self::H264 => EncodedVideoCodec::H264,
        }
    }
}

#[derive(Clone, Serialize)]
pub struct Encoder {
    pub factory: String,
    pub label: String,
    pub codec: Codec,
}

pub fn encoders() -> Vec<Encoder> {
    [
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

#[derive(Clone, Copy)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u64,
}

impl Mode {
    pub fn baseline(source: &Source) -> Result<Self> {
        let scale = (1920.0 / source.info.width as f64)
            .min(1080.0 / source.info.height as f64)
            .min(1.0);
        let width = (source.info.width as f64 * scale) as u32 / 2 * 2;
        let height = (source.info.height as f64 * scale) as u32 / 2 * 2;
        ensure!(
            width >= 144 && height >= 128,
            "Choose a larger source window."
        );
        Ok(Self {
            width,
            height,
            fps: 60,
            bitrate: 12_000_000,
        })
    }
}

#[derive(Default, Clone, Serialize)]
pub struct Metrics {
    pub frames: u64,
    pub accepted: u64,
    pub bytes: u64,
    pub fps: f64,
    pub encoder_bitrate: u64,
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
}

impl Drop for Encoding {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

impl Encoding {
    pub fn codec(&self) -> Codec {
        self.codec
    }
    pub fn start(
        source: &Source,
        encoder: &Encoder,
        mode: Mode,
        stop: &AtomicBool,
    ) -> Result<Self> {
        ensure!(!stop.load(Ordering::Acquire), "Capture was cancelled.");
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
        let (parser, media_type, profile) = match encoder.codec {
            Codec::H265 => ("h265parse", "video/x-h265", "main"),
            Codec::H264 => ("h264parse", "video/x-h264", "constrained-baseline"),
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
            "queue name=entry max-size-buffers=2 max-size-bytes=0 max-size-time=0 leaky=downstream ! {cadence}d3d11convert ! video/x-raw(memory:D3D11Memory),format=NV12,width={},height={},framerate={}/1 ! {} name=hardware bitrate={} gop-size={} ! {media_type},profile={profile} ! {parser} config-interval=-1 ! {media_type},stream-format=byte-stream,alignment=au ! appsink name=encoded max-buffers=2 sync=false",
            mode.width,
            mode.height,
            mode.fps,
            encoder.factory,
            mode.bitrate / 1000,
            mode.fps
        );
        let pipeline = gst::parse::launch(&processing)
            .context("Could not create the GPU encoding pipeline.")?
            .downcast::<gst::Pipeline>()
            .map_err(|_| anyhow::anyhow!("Invalid encoding pipeline."))?;
        let input = match source.target {
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
        input
            .link(
                &pipeline
                    .by_name("entry")
                    .context("Missing capture queue.")?,
            )
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
            hardware.set_property_from_str("usage", "ultra-low-latency");
            hardware.set_property_from_str("rate-control", "cbr");
        }
        let sink = pipeline
            .by_name("encoded")
            .context("Missing encoded frame sink.")?
            .downcast::<AppSink>()
            .map_err(|_| anyhow::anyhow!("Invalid frame sink."))?;
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
            encoder_bitrate: self.mode.bitrate,
            ..Default::default()
        };
        let mut keyframe_count = 0;
        let mut waiting_for_key = true;
        let mut last_key_request = Instant::now() - Duration::from_secs(1);
        while !stop.load(Ordering::Acquire) {
            self.check_bus()?;
            if let Some(source) = &source {
                if source.take_keyframe_request()
                    || (waiting_for_key && last_key_request.elapsed() >= Duration::from_millis(500))
                {
                    keyframe_count += 1;
                    self.keyframe(keyframe_count);
                    last_key_request = Instant::now();
                }
                if let Some(target) = source.take_rate_control_request() {
                    let bitrate = target.target_bitrate_bps.min(self.mode.bitrate).max(1_000);
                    self.encoder
                        .set_property("bitrate", (bitrate / 1000) as u32);
                    counters.encoder_bitrate = bitrate;
                }
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
                    codec: self.codec.encoded(),
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
