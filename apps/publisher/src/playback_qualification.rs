//! Explicit, local process-loopback acceptance; never used by normal playback.
//! Only numeric timing aggregates leave this module. No microphone or other
//! application's audio is opened, and no audio samples are retained.
use anyhow::{Context, Result, ensure};
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::{AppSink, AppSrc};
use std::time::{Duration, Instant};

pub fn run(legacy: bool) -> Result<()> {
    let output = crate::playback::audio_pipeline()?;
    let input = output
        .0
        .by_name("pcm")
        .context("Missing playback input.")?
        .downcast::<AppSrc>()
        .map_err(|_| anyhow::anyhow!("Invalid playback input."))?;
    let device = output
        .0
        .by_name("device")
        .context("Missing playback device.")?;
    if legacy {
        device.set_property("buffer-time", 200_000_i64);
    }
    let capture = gst::parse::launch(&format!(
        "wasapi2src name=capture loopback=true loopback-mode=include-process-tree loopback-target-pid={} low-latency=true provide-clock=false ! audioconvert ! audioresample ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! appsink name=observed max-buffers=4 drop=true sync=false",
        std::process::id()
    ))?
    .downcast::<gst::Pipeline>()
    .map_err(|_| anyhow::anyhow!("Invalid process-loopback pipeline."))?;
    let capture = crate::playback::AudioPipeline(capture);
    let observed = capture
        .0
        .by_name("observed")
        .context("Missing process-loopback output.")?
        .downcast::<AppSink>()
        .map_err(|_| anyhow::anyhow!("Invalid process-loopback output."))?;
    let source = capture
        .0
        .by_name("capture")
        .context("Missing process-loopback source.")?;
    capture.0.set_state(gst::State::Playing)?;
    output.0.set_state(gst::State::Playing)?;
    let started = Instant::now();
    let mut pushed = Vec::new();
    let mut heard = Vec::new();
    let mut capture_ages = Vec::new();
    let mut last_active: Option<Instant> = None;
    let mut max_input_bytes = 0;
    for block in 0..1240_u64 {
        std::thread::sleep(
            (started + Duration::from_millis(block * 10)).saturating_duration_since(Instant::now()),
        );
        for pipeline in [&output.0, &capture.0] {
            ensure!(
                pipeline
                    .bus()
                    .is_none_or(|bus| bus.pop_filtered(&[gst::MessageType::Error]).is_none()),
                "Playback loopback qualification failed."
            );
        }
        if block < 1200 {
            let impulse = block >= 100 && block % 100 < 2;
            let samples = (0..480)
                .flat_map(|position| {
                    let value = if impulse {
                        (3000.0
                            * (std::f64::consts::TAU * 1000.0 * position as f64 / 48000.0).sin())
                            as i16
                    } else {
                        0
                    };
                    [value, value]
                })
                .collect();
            if impulse && block % 100 == 0 {
                pushed.push(Instant::now());
            }
            input.push_buffer(crate::playback::audio_buffer(samples))?;
            max_input_bytes = max_input_bytes.max(input.current_level_bytes());
        }
        while let Some(sample) = observed.try_pull_sample(gst::ClockTime::ZERO) {
            let buffer = sample.buffer().context("Missing loopback samples.")?;
            let map = buffer.map_readable()?;
            let active = map
                .chunks_exact(2)
                .any(|s| i16::from_le_bytes([s[0], s[1]]).unsigned_abs() > 600);
            let now = Instant::now();
            if active {
                if last_active
                    .is_none_or(|last| now.duration_since(last) > Duration::from_millis(300))
                {
                    heard.push(now);
                    if let Some(age) = capture
                        .0
                        .current_running_time()
                        .zip(buffer.pts())
                        .and_then(|(now, pts)| now.checked_sub(pts))
                    {
                        capture_ages.push(age.nseconds() as f64 / 1e6);
                    }
                }
                last_active = Some(now);
            }
        }
    }
    ensure!(
        pushed.len() == 11 && heard.len() == pushed.len(),
        "Expected eleven process-loopback impulses; no timing result is valid."
    );
    let delays: Vec<_> = heard
        .iter()
        .zip(&pushed)
        .map(|(heard, pushed)| heard.saturating_duration_since(*pushed).as_secs_f64() * 1000.0)
        .collect();
    ensure!(
        delays.iter().all(|delay| *delay > 0.0 && *delay < 900.0),
        "Loopback impulses could not be paired unambiguously."
    );
    println!(
        "{}",
        serde_json::json!({
            "sinkBufferUs": device.property::<i64>("buffer-time"),
            "captureBufferUs": source.property::<i64>("actual-buffer-time"),
            "capturePeriodUs": source.property::<i64>("actual-latency-time"),
            "maxInputBytes": max_input_bytes,
            "delaysMs": delays,
            "captureAgesMs": capture_ages,
            "physicalSpeakerTiming": "unavailable",
        })
    );
    Ok(())
}
