//! Local-only native D3D11 presentation experiment. No capture, room, audio or network.
//!
//! GPU mode retains D3D11Memory from the hardware decoder to d3d11videosink.
//! Readback mode explicitly downloads NV12, converts via CPU I420 then RGBA,
//! uploads RGBA, and uses the identical HWND/sink. This isolates that pipeline;
//! it does not reproduce the SDK's I420 copies, Rust ColorImage, or egui upload.
//!
//! Args: gpu|readback h264|h265|av1 fixture width height fps frames report.json
//!       display-width display-height
//! Uses the installed/pinned runtime through PATH. A GUI fixture window appears;
//! the executable never creates a console. The same fixture must be used for both modes.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("This experiment requires Windows Direct3D11.");
}

#[cfg(windows)]
fn main() {
    let output = std::env::args_os().nth(8).map(std::path::PathBuf::from);
    let result = experiment::run();
    if let Err(error) = result {
        // Do not dump GStreamer messages, environment, input paths, or SDK logs.
        if let Some(output) = output {
            let _ = std::fs::write(
                output,
                serde_json::to_vec_pretty(&serde_json::json!({
                    "status": "failed", "error": error.to_string()
                }))
                .unwrap_or_default(),
            );
        }
        std::process::exit(1);
    }
    if result.is_ok_and(|valid| !valid) {
        std::process::exit(2);
    }
}

#[cfg(windows)]
mod experiment {
    use anyhow::{Context, Result, bail, ensure};
    use gstreamer::{self as gst, prelude::*};
    use gstreamer_video::{VideoOverlay, prelude::*};
    use serde_json::{Value, json};
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };
    use windows::{
        Win32::{
            Foundation::{FILETIME, HWND, RECT},
            System::Threading::{GetCurrentProcess, GetProcessTimes},
            UI::WindowsAndMessaging::{
                AdjustWindowRect, CreateWindowExW, DestroyWindow, DispatchMessageW, IsWindow, MSG,
                PM_REMOVE, PeekMessageW, TranslateMessage, WINDOW_EX_STYLE, WS_CAPTION, WS_CHILD,
                WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_OVERLAPPED, WS_SYSMENU, WS_VISIBLE,
            },
        },
        core::{PCWSTR, w},
    };

    struct Workload {
        mode: String,
        codec: String,
        fixture: PathBuf,
        width: u32,
        height: u32,
        fps: u32,
        frames: u64,
        output: PathBuf,
        display_width: i32,
        display_height: i32,
    }

    impl Workload {
        fn arguments() -> Result<Self> {
            let args: Vec<_> = std::env::args().skip(1).collect();
            ensure!(args.len() == 10, "Expected ten experiment arguments.");
            let workload = Self {
                mode: args[0].clone(),
                codec: args[1].clone(),
                fixture: PathBuf::from(&args[2]),
                width: args[3].parse().context("Invalid source width.")?,
                height: args[4].parse().context("Invalid source height.")?,
                fps: args[5].parse().context("Invalid source FPS.")?,
                frames: args[6].parse().context("Invalid expected frame count.")?,
                output: PathBuf::from(&args[7]),
                display_width: args[8].parse().context("Invalid display width.")?,
                display_height: args[9].parse().context("Invalid display height.")?,
            };
            ensure!(
                matches!(workload.mode.as_str(), "gpu" | "readback"),
                "Invalid mode."
            );
            ensure!(
                matches!(workload.codec.as_str(), "h264" | "h265" | "av1"),
                "Invalid codec."
            );
            ensure!(
                workload.fixture.is_file(),
                "Local elementary-stream fixture is missing."
            );
            ensure!(
                (144..=3840).contains(&workload.width)
                    && (144..=2160).contains(&workload.height)
                    && (1..=120).contains(&workload.fps)
                    && (1..=72000).contains(&workload.frames)
                    && (144..=3840).contains(&workload.display_width)
                    && (144..=2160).contains(&workload.display_height),
                "Workload exceeds local experiment bounds."
            );
            ensure!(
                workload.fixture != workload.output,
                "Report cannot replace the fixture."
            );
            Ok(workload)
        }
    }

    struct Window(HWND);

    impl Window {
        fn parent(width: i32, height: i32) -> Result<Self> {
            let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE | WS_CLIPCHILDREN;
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            unsafe { AdjustWindowRect(&mut rect, style, false) }
                .context("Cannot calculate fixture viewport.")?;
            let hwnd = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    w!("FreedomStream local GPU fixture"),
                    style,
                    40,
                    40,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .context("Cannot create fixture viewport.")?;
            Ok(Self(hwnd))
        }

        fn child(parent: HWND, width: i32, height: i32) -> Result<Self> {
            let hwnd = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                    0,
                    0,
                    width,
                    height,
                    Some(parent),
                    None,
                    None,
                    None,
                )
            }
            .context("Cannot create native presentation child.")?;
            Ok(Self(hwnd))
        }
    }

    impl Drop for Window {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }

    #[derive(Default)]
    struct Counters {
        decoded: u64,
        sink_buffers: u64,
        sink_d3d11: u64,
        decoded_d3d11: u64,
        downloaded: u64,
        downloaded_system_memory: u64,
        cpu_rgba: u64,
        cpu_rgba_system_memory: u64,
        cpu_rgba_bytes: u64,
        invalid_pts: u64,
        first_pts: Option<u64>,
        last_pts: Option<u64>,
        last_sink_at: Option<Instant>,
        sink_intervals_us: Vec<u64>,
        decoded_at: HashMap<u64, Instant>,
        transfer_us: Vec<u64>,
        decoder_caps: Option<String>,
        sink_caps: Option<String>,
        cpu_caps: Option<String>,
    }

    // Minimal experiment interface: start(host, workload), poll(), and Drop.
    // The host owns/message-pumps its HWND and outlives this object. This module
    // owns the child, decoder, format negotiation, bounded queues and stop order.
    struct Presentation {
        pipeline: gst::Pipeline,
        sink: gst::Element,
        counters: Arc<Mutex<Counters>>,
        started: Instant,
        initial_cpu_ms: f64,
        _child: Window,
    }

    impl Presentation {
        fn start(host: HWND, workload: &Workload) -> Result<Self> {
            let child = Window::child(host, workload.display_width, workload.display_height)?;
            let parser = match workload.codec.as_str() {
                "h264" => "h264parse",
                "h265" => "h265parse",
                "av1" => "av1parse",
                _ => unreachable!(),
            };
            let decoder = format!("d3d11{}dec", workload.codec);
            let fixture_clock = if workload.codec == "av1" {
                format!(
                    " ! capssetter caps=\"video/x-raw,framerate={}/1\"",
                    workload.fps
                )
            } else {
                String::new()
            };
            let input_caps = if workload.codec == "av1" {
                format!(" ! video/x-av1,framerate={}/1", workload.fps)
            } else {
                String::new()
            };
            let readback = if workload.mode == "readback" {
                " ! d3d11download name=download ! video/x-raw,format=NV12 \
                  ! videoconvert ! video/x-raw,format=I420 \
                  ! videoconvert name=cpu_rgba ! video/x-raw,format=RGBA \
                  ! d3d11upload ! video/x-raw(memory:D3D11Memory),format=RGBA"
            } else {
                ""
            };
            let description = format!(
                "filesrc name=fixture {input_caps} ! {parser} \
                 ! {decoder} name=decoder {fixture_clock} \
                 ! video/x-raw(memory:D3D11Memory),format=NV12,width={},height={},framerate={}/1 \
                 {readback} \
                 ! d3d11videosink name=present sync=true qos=false max-lateness=-1 \
                   processing-deadline=0 enable-last-sample=false force-aspect-ratio=true",
                workload.width, workload.height, workload.fps,
            );
            let pipeline = gst::parse::launch(&description)
                .map_err(|_| anyhow::anyhow!("Cannot construct the selected native pipeline."))?
                .downcast::<gst::Pipeline>()
                .map_err(|_| anyhow::anyhow!("The native pipeline has an unexpected type."))?;
            pipeline
                .by_name("fixture")
                .context("Missing fixture source.")?
                .set_property("location", workload.fixture.to_string_lossy().as_ref());
            let sink = pipeline
                .by_name("present")
                .context("Missing native video sink.")?;
            let overlay = sink
                .clone()
                .dynamic_cast::<VideoOverlay>()
                .map_err(|_| anyhow::anyhow!("Native sink lacks video-overlay support."))?;
            // The child is valid on the pumping thread and remains valid until
            // after the pipeline reaches Null. No GPU buffer is mapped by Rust.
            unsafe {
                overlay.set_window_handle(child.0.0 as usize);
            }
            overlay.handle_events(false);
            if workload.codec == "av1" {
                // AV1 elementary files have no container/RTP clock and the
                // parser reports 0/1. Assign the fixture's declared source
                // cadence; this is metadata-only and does not map GPU memory.
                let sequence = std::sync::atomic::AtomicU64::new(0);
                let fps = u64::from(workload.fps);
                pipeline
                    .by_name("decoder")
                    .and_then(|element| element.static_pad("src"))
                    .context("Missing fixture clock pad.")?
                    .add_probe(gst::PadProbeType::BUFFER, move |_, info| {
                        if let Some(gst::PadProbeData::Buffer(buffer)) = &mut info.data {
                            let index = sequence.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let buffer = buffer.make_mut();
                            buffer.set_pts(gst::ClockTime::from_nseconds(
                                index * 1_000_000_000 / fps,
                            ));
                            buffer.set_duration(gst::ClockTime::from_nseconds(1_000_000_000 / fps));
                        }
                        gst::PadProbeReturn::Ok
                    });
            }
            let counters = Arc::new(Mutex::new(Counters::default()));
            add_probe(
                &pipeline,
                "decoder",
                "src",
                counters.clone(),
                Probe::Decoded,
            )?;
            add_probe(&pipeline, "present", "sink", counters.clone(), Probe::Sink)?;
            if workload.mode == "readback" {
                add_probe(
                    &pipeline,
                    "download",
                    "src",
                    counters.clone(),
                    Probe::Downloaded,
                )?;
                add_probe(
                    &pipeline,
                    "cpu_rgba",
                    "src",
                    counters.clone(),
                    Probe::CpuRgba,
                )?;
            }
            let presentation = Self {
                pipeline,
                sink,
                counters,
                started: Instant::now(),
                initial_cpu_ms: cpu_ms()?,
                _child: child,
            };
            presentation
                .pipeline
                .set_state(gst::State::Playing)
                .context("Native pipeline could not start.")?;
            Ok(presentation)
        }

        fn poll(&self) -> Result<Option<Value>> {
            let bus = self
                .pipeline
                .bus()
                .context("Missing native pipeline bus.")?;
            while let Some(message) = bus.pop() {
                match message.view() {
                    gst::MessageView::Error(_) => {
                        bail!("Native pipeline reported an error; no fallback was used.")
                    }
                    gst::MessageView::Eos(_) => {
                        let stats = self.sink.property::<gst::Structure>("stats");
                        let rendered = stats.get::<u64>("rendered").unwrap_or(0);
                        let dropped = stats.get::<u64>("dropped").unwrap_or(0);
                        let cpu_ms = cpu_ms()? - self.initial_cpu_ms;
                        let elapsed_ms = self.started.elapsed().as_secs_f64() * 1000.0;
                        let count = self
                            .counters
                            .lock()
                            .map_err(|_| anyhow::anyhow!("Counters unavailable."))?;
                        return Ok(Some(json!({
                            "wallMs": elapsed_ms,
                            "processCpuMs": cpu_ms,
                            "processCpuPercentOfOneCore": cpu_ms / elapsed_ms * 100.0,
                            "decodedFrames": count.decoded,
                            "decodedD3d11Buffers": count.decoded_d3d11,
                            "sinkBuffers": count.sink_buffers,
                            "sinkD3d11Buffers": count.sink_d3d11,
                            "renderedFrames": rendered,
                            "droppedFrames": dropped,
                            "downloadedFrames": count.downloaded,
                            "downloadedSystemMemoryBuffers": count.downloaded_system_memory,
                            "cpuRgbaFrames": count.cpu_rgba,
                            "cpuRgbaSystemMemoryBuffers": count.cpu_rgba_system_memory,
                            "cpuRgbaBufferBytes": count.cpu_rgba_bytes,
                            "invalidPtsFrames": count.invalid_pts,
                            "firstPtsNs": count.first_pts,
                            "lastPtsNs": count.last_pts,
                            "decodedToSinkProbeUs": percentiles(&count.transfer_us),
                            "sinkArrivalIntervalsUs": percentiles(&count.sink_intervals_us),
                            "decoderCaps": count.decoder_caps,
                            "sinkCaps": count.sink_caps,
                            "cpuRgbaCaps": count.cpu_caps,
                            "unmatchedDecodeTimestamps": count.decoded_at.len(),
                        })));
                    }
                    _ => {}
                }
            }
            Ok(None)
        }
    }

    impl Drop for Presentation {
        fn drop(&mut self) {
            // State Null joins sink/decoder work before the child HWND drops.
            let _ = self.pipeline.set_state(gst::State::Null);
        }
    }

    #[derive(Clone, Copy)]
    enum Probe {
        Decoded,
        Sink,
        Downloaded,
        CpuRgba,
    }

    fn add_probe(
        pipeline: &gst::Pipeline,
        name: &str,
        pad: &str,
        counters: Arc<Mutex<Counters>>,
        kind: Probe,
    ) -> Result<()> {
        let pad = pipeline
            .by_name(name)
            .context("Missing measured pipeline stage.")?
            .static_pad(pad)
            .context("Missing measured pipeline pad.")?;
        pad.add_probe(gst::PadProbeType::BUFFER, move |pad, info| {
            if let Some(buffer) = info.buffer() {
                let now = Instant::now();
                let Ok(mut count) = counters.lock() else {
                    return gst::PadProbeReturn::Ok;
                };
                // Inspect allocator identity only: mapping a GPU buffer would
                // itself force download and invalidate this experiment.
                let d3d11 = buffer.n_memory() != 0
                    && buffer
                        .iter_memories()
                        .all(|memory| memory.is_type("D3D11Memory"));
                let system_memory = buffer.n_memory() != 0
                    && buffer
                        .iter_memories()
                        .all(|memory| memory.is_type("SystemMemory"));
                let pts = buffer.pts().map(|pts| pts.nseconds());
                match kind {
                    Probe::Decoded => {
                        count.decoded += 1;
                        count.decoded_d3d11 += u64::from(d3d11);
                        if count.decoder_caps.is_none() {
                            count.decoder_caps = pad.current_caps().map(|caps| caps.to_string());
                        }
                        if let Some(pts) = pts {
                            if count.decoded_at.len() < 256 {
                                count.decoded_at.insert(pts, now);
                            }
                        }
                    }
                    Probe::Sink => {
                        count.sink_buffers += 1;
                        count.sink_d3d11 += u64::from(d3d11);
                        if count.sink_caps.is_none() {
                            count.sink_caps = pad.current_caps().map(|caps| caps.to_string());
                        }
                        if let Some(pts) = pts {
                            count.first_pts.get_or_insert(pts);
                            count.last_pts = Some(pts);
                            if let Some(started) = count.decoded_at.remove(&pts) {
                                count
                                    .transfer_us
                                    .push(now.duration_since(started).as_micros() as u64);
                            }
                        } else {
                            count.invalid_pts += 1;
                        }
                        if let Some(last) = count.last_sink_at {
                            count
                                .sink_intervals_us
                                .push(now.duration_since(last).as_micros() as u64);
                        }
                        count.last_sink_at = Some(now);
                    }
                    Probe::Downloaded => {
                        count.downloaded += 1;
                        count.downloaded_system_memory += u64::from(system_memory);
                    }
                    Probe::CpuRgba => {
                        count.cpu_rgba += 1;
                        count.cpu_rgba_system_memory += u64::from(system_memory);
                        count.cpu_rgba_bytes += buffer.size() as u64;
                        if count.cpu_caps.is_none() {
                            count.cpu_caps = pad.current_caps().map(|caps| caps.to_string());
                        }
                    }
                }
            }
            gst::PadProbeReturn::Ok
        })
        .context("Cannot install native pipeline counter.")?;
        Ok(())
    }

    fn cpu_ms() -> Result<f64> {
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
        }
        .context("Process CPU counter unavailable.")?;
        let ticks = |value: FILETIME| {
            (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
        };
        Ok((ticks(kernel) + ticks(user)) as f64 / 10_000.0)
    }

    fn percentiles(samples: &[u64]) -> Value {
        if samples.is_empty() {
            return Value::Null;
        }
        let mut values = samples.to_vec();
        values.sort_unstable();
        let at = |p: usize| values[(values.len() - 1) * p / 100];
        json!({"count": values.len(), "p50": at(50), "p95": at(95), "p99": at(99), "max": values.last()})
    }

    fn pump_messages() {
        let mut message = MSG::default();
        unsafe {
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    pub fn run() -> Result<bool> {
        let workload = Workload::arguments()?;
        gst::init().map_err(|_| anyhow::anyhow!("Native media runtime initialization failed."))?;
        let parent = Window::parent(workload.display_width, workload.display_height)?;
        let presentation = Presentation::start(parent.0, &workload)?;
        let deadline =
            Instant::now() + Duration::from_secs(workload.frames / u64::from(workload.fps) + 15);
        let mut report = loop {
            pump_messages();
            ensure!(
                unsafe { IsWindow(Some(parent.0)) }.as_bool(),
                "Fixture window was closed before completion."
            );
            if let Some(report) = presentation.poll()? {
                break report;
            }
            ensure!(
                Instant::now() < deadline,
                "Native pipeline exceeded its bounded deadline."
            );
            std::thread::sleep(Duration::from_millis(2));
        };
        let expected = workload.frames;
        let decoded = report["decodedFrames"].as_u64().unwrap_or(0);
        let sink = report["sinkBuffers"].as_u64().unwrap_or(0);
        let rendered = report["renderedFrames"].as_u64().unwrap_or(0);
        let d3d11 = report["sinkD3d11Buffers"].as_u64().unwrap_or(0);
        let decoder_d3d11 = report["decodedD3d11Buffers"].as_u64().unwrap_or(0);
        let expected_span_ns = (expected - 1) * 1_000_000_000 / u64::from(workload.fps);
        let pts_span_ns = report["lastPtsNs"]
            .as_u64()
            .zip(report["firstPtsNs"].as_u64())
            .and_then(|(last, first)| last.checked_sub(first));
        let pts_span_valid = pts_span_ns.is_some_and(|span| {
            span.abs_diff(expected_span_ns) <= (expected_span_ns / 100).max(1_000_000)
        });
        let expected_ms = expected as f64 * 1000.0 / f64::from(workload.fps);
        let wall_ms = report["wallMs"].as_f64().unwrap_or(0.0);
        let paced = pts_span_valid
            && wall_ms >= expected_ms * 0.90
            && wall_ms <= expected_ms * 1.25 + 1000.0;
        let valid = decoded == expected
            && sink == expected
            && rendered == expected
            && d3d11 == expected
            && decoder_d3d11 == expected
            && paced
            && report["invalidPtsFrames"].as_u64() == Some(0)
            && report["droppedFrames"].as_u64() == Some(0)
            && (workload.mode == "gpu"
                || (report["downloadedFrames"].as_u64() == Some(expected)
                    && report["downloadedSystemMemoryBuffers"].as_u64() == Some(expected)
                    && report["cpuRgbaSystemMemoryBuffers"].as_u64() == Some(expected)
                    && report["cpuRgbaFrames"].as_u64() == Some(expected)));
        report["status"] = json!(if valid {
            "completed"
        } else {
            "invalid-comparison"
        });
        report["mode"] = json!(workload.mode);
        report["codec"] = json!(workload.codec);
        report["width"] = json!(workload.width);
        report["height"] = json!(workload.height);
        report["fps"] = json!(workload.fps);
        report["expectedFrames"] = json!(expected);
        report["displayWidth"] = json!(workload.display_width);
        report["displayHeight"] = json!(workload.display_height);
        report["runtime"] = json!(gst::version_string().to_string());
        report["expectedWallMs"] = json!(expected_ms);
        report["expectedPtsSpanNs"] = json!(expected_span_ns);
        report["actualPtsSpanNs"] = json!(pts_span_ns);
        report["pacingVerified"] = json!(paced);
        report["fixtureBytes"] = json!(
            std::fs::metadata(&workload.fixture)
                .context("Fixture metadata unavailable.")?
                .len()
        );
        report["measurementLimits"] = json!([
            "Isolated elementary-stream pipeline; not the LiveKit/Rust/egui receive path.",
            "BaseSink rendered counts native render calls, not observed display photons.",
            "decodedToSinkProbeUs excludes decode, clock wait and actual GPU presentation completion.",
            "cpuRgbaBufferBytes is logical buffer size, not measured PCIe transfer volume.",
            "Process CPU includes startup and the Win32 message loop; GPU engine time is external.",
            "Unbounded lateness is allowed; compare wall time and PTS span against frame rate."
        ]);
        write_report(&workload.output, &report)?;
        drop(presentation);
        drop(parent);
        Ok(valid)
    }

    fn write_report(path: &Path, report: &Value) -> Result<()> {
        let data =
            serde_json::to_vec_pretty(report).context("Cannot serialize comparison report.")?;
        std::fs::write(path, data).context("Cannot write comparison report.")
    }
}
