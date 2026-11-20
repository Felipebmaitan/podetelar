//! Local synthetic pipeline probe; never part of the distributed app.
#![cfg_attr(windows, windows_subsystem = "windows")]
use anyhow::{Result, ensure};
use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSink;
use serde::Deserialize;
use serde_json::json;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    description: String,
    #[serde(default)]
    appsink: Option<String>,
    #[serde(default)]
    expected_frames: Option<u64>,
    #[serde(default)]
    stall_at: u64,
    #[serde(default)]
    stall_ms: u64,
    #[serde(default)]
    seconds: u64,
    #[serde(default)]
    pcm: bool,
}
fn run(request: Request) -> Result<serde_json::Value> {
    ensure!(
        request.description.len() < 16384 && request.stall_ms <= 2000 && request.seconds <= 120,
        "Invalid local probe."
    );
    gst::init()?;
    let graph = gst::parse::launch(&request.description)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow::anyhow!("Not a pipeline."))?;
    let sink = request
        .appsink
        .as_ref()
        .map(|name| {
            graph
                .by_name(name)
                .ok_or_else(|| anyhow::anyhow!("Missing sink."))?
                .downcast::<AppSink>()
                .map_err(|_| anyhow::anyhow!("Invalid sink."))
        })
        .transpose()?;
    let delivered = Arc::new(AtomicU64::new(0));
    if let Some(element) = graph.by_name("count") {
        let counter = delivered.clone();
        element
            .static_pad("sink")
            .ok_or_else(|| anyhow::anyhow!("No counter pad."))?
            .add_probe(gst::PadProbeType::BUFFER, move |_, _| {
                counter.fetch_add(1, Ordering::Relaxed);
                gst::PadProbeReturn::Ok
            });
    }
    let bus = graph.bus().ok_or_else(|| anyhow::anyhow!("Missing bus."))?;
    let start = Instant::now();
    graph.set_state(gst::State::Playing)?;
    let result = (|| -> Result<_> {
        let mut pulled = 0u64;
        let mut bytes = 0u64;
        let mut max_age = 0.0f64;
        let mut pcm_energy = [0.0f64; 2];
        let mut pcm_peak = [0.0f64; 2];
        let mut pcm_frames = 0u64;
        let mut eos = false;
        let mut first_pts = None;
        let mut last_pts = None;
        loop {
            if let Some(sink) = &sink
                && let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_mseconds(10))
            {
                pulled += 1;
                if let Some(buffer) = sample.buffer() {
                    bytes += buffer.size() as u64;
                    first_pts = first_pts.or(buffer.pts());
                    last_pts = buffer.pts();
                    if let Some(age) = graph
                        .current_running_time()
                        .zip(buffer.pts())
                        .and_then(|(now, pts)| now.checked_sub(pts))
                    {
                        max_age = max_age.max(age.nseconds() as f64 / 1e6);
                    }
                    if request.pcm {
                        let map = buffer.map_readable()?;
                        for frame in map.chunks_exact(4) {
                            for ch in 0..2 {
                                let n = i16::from_le_bytes([frame[ch * 2], frame[ch * 2 + 1]])
                                    as f64
                                    / 32768.0;
                                pcm_energy[ch] += n * n;
                                pcm_peak[ch] = pcm_peak[ch].max(n.abs());
                            }
                            pcm_frames += 1;
                        }
                    }
                }
                if pulled == request.stall_at {
                    std::thread::sleep(Duration::from_millis(request.stall_ms));
                }
            }
            while let Some(message) = bus.pop() {
                match message.view() {
                    gst::MessageView::Error(_) => anyhow::bail!("Synthetic pipeline error."),
                    gst::MessageView::Eos(_) => eos = true,
                    _ => {}
                }
            }
            if eos && sink.as_ref().is_none_or(|sink| sink.is_eos()) {
                break;
            }
            if request.seconds > 0 && start.elapsed() >= Duration::from_secs(request.seconds) {
                break;
            }
            ensure!(
                start.elapsed() < Duration::from_secs(request.seconds.max(70) + 5),
                "Synthetic pipeline timeout."
            );
            if sink.is_none() {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        let retained = sink
            .as_ref()
            .and_then(|sink| sink.property::<Option<gst::Sample>>("last-sample"))
            .and_then(|sample| sample.buffer().map(|b| b.size()))
            .unwrap_or(0);
        let valid = request
            .expected_frames
            .is_none_or(|expected| expected == pulled);
        Ok(
            json!({"schema":1,"completed":true,"valid":valid,"eos":eos,"wallMs":start.elapsed().as_secs_f64()*1000.0,"pulledFrames":pulled,"deliveredBuffers":delivered.load(Ordering::Relaxed),"logicalBytes":bytes,"lastSampleRetainedBytes":retained,"maxRunningTimeMinusPtsMs":max_age,"firstPtsNs":first_pts.map(|t|t.nseconds()),"lastPtsNs":last_pts.map(|t|t.nseconds()),"pcmFrames":pcm_frames,"pcmRms":pcm_energy.map(|n|if pcm_frames>0{(n/pcm_frames as f64).sqrt()}else{0.0}),"pcmPeak":pcm_peak,"limits":"Synthetic pipeline, not SDK, app, physical latency or audible acceptance. PTS age meaningful only for a live correctly timestamped source. Buffer count is not physical presentation."}),
        )
    })();
    let _ = graph.set_state(gst::State::Null);
    result
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        std::process::exit(2);
    }
    let result = (|| -> Result<_> {
        let bytes = std::fs::read(&args[1])?;
        ensure!(bytes.len() <= 16384, "Oversized probe.");
        run(serde_json::from_slice(&bytes)?)
    })();
    match result {
        Ok(value) => {
            if std::fs::write(&args[2], serde_json::to_vec_pretty(&value).unwrap()).is_err() {
                std::process::exit(2);
            }
        }
        Err(_) => {
            let _ = std::fs::write(
                &args[2],
                b"{\"completed\":false,\"error\":\"local pipeline failed\"}",
            );
            std::process::exit(1);
        }
    }
}
