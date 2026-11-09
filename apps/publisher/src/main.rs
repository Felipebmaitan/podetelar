mod admission;
mod audio;
mod diagnostics;
mod encoding;
mod preview;
mod publisher;
mod sources;
mod ui;
mod viewer;
#[cfg(windows)]
mod win32;

use anyhow::{Context, Result, bail, ensure};
use encoding::{Codec, Encoding, Metrics, Mode};
use livekit::prelude::*;
use serde::Deserialize;
use std::{
    io::Read,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

fn main() {
    if let Err(error) = run() {
        // Only application errors reach here; network/token-bearing SDK errors
        // are deliberately replaced with bounded messages at admission/publication.
        eprintln!("FreedomStream: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
        };
        // Before any HWND or capture enumeration: source bounds must use physical
        // pixels, including mixed-DPI desktops. A packaged manifest may set this first.
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    if std::env::var("FREEDOMSTREAM_DIAGNOSTICS").as_deref() == Ok("1") {
        diagnostics::enable();
    }
    gstreamer::init()
        .context("GStreamer could not initialize. Check the native runtime installation.")?;
    match std::env::args().nth(1).as_deref() {
        Some("--probe") => println!(
            "{}",
            serde_json::json!({"encoders": encoding::encoders(), "sourceCount": sources::enumerate().len(), "runtime": gstreamer::version_string().to_string()})
        ),
        Some("--sources") => println!(
            "{}",
            serde_json::to_string(
                &sources::enumerate()
                    .into_iter()
                    .map(|source| source.info)
                    .collect::<Vec<_>>()
            )?
        ),
        Some("--audio-apps") => println!("{}", serde_json::to_string(&audio::applications()?)?),
        Some("--audio-tone") => {
            use gstreamer::prelude::*;
            let frequency: u32 = std::env::args()
                .nth(2)
                .context("Choose a test tone.")?
                .parse()?;
            ensure!(
                [440, 880, 1320].contains(&frequency),
                "Use a supported test tone."
            );
            let description = if frequency == 440 {
                "audiomixer name=mix ! audioconvert ! audio/x-raw,rate=48000,channels=2 ! wasapi2sink low-latency=true audiotestsrc is-live=true freq=440 volume=0.02 samplesperbuffer=480 ! audio/x-raw,format=F32LE,rate=48000,channels=1 ! audiopanorama panorama=-1 ! mix. audiotestsrc is-live=true freq=660 volume=0.02 samplesperbuffer=480 ! audio/x-raw,format=F32LE,rate=48000,channels=1 ! audiopanorama panorama=1 ! mix.".to_owned()
            } else {
                format!(
                    "audiotestsrc is-live=true freq={frequency} volume=0.02 samplesperbuffer=480 ! audio/x-raw,rate=48000,channels=2 ! wasapi2sink low-latency=true"
                )
            };
            let pipeline = gstreamer::parse::launch(&description)?
                .downcast::<gstreamer::Pipeline>()
                .map_err(|_| anyhow::anyhow!("Invalid audio fixture."))?;
            pipeline.set_state(gstreamer::State::Playing)?;
            std::thread::sleep(Duration::from_secs(45));
            pipeline.set_state(gstreamer::State::Null)?;
        }
        Some("--qualify-audio") => {
            let mut input = String::new();
            std::io::stdin().take(8193).read_to_string(&mut input)?;
            ensure!(input.len() <= 8192, "Audio fixture input is too large.");
            let request: AudioRequest =
                serde_json::from_str(&input).context("Invalid audio fixture input.")?;
            ensure!(
                (1..=20).contains(&request.seconds),
                "Audio qualification duration must be between 1 and 20 seconds."
            );
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            let metrics = runtime.block_on(async {
                let mut viewer = match request.viewer_invitation {
                    Some(invitation) => {
                        let mut viewer = viewer::Viewer::open(&invitation, true).await?;
                        viewer.qualification_tone().await?;
                        Some(viewer)
                    }
                    None => None,
                };
                let result = tokio::task::spawn_blocking(move || {
                    audio::run(
                        request.choice,
                        request.exclusions,
                        Arc::new(AtomicBool::new(false)),
                        Arc::new(Mutex::new(audio::Metrics::default())),
                        None,
                        Some(Duration::from_secs(request.seconds)),
                    )
                })
                .await
                .context("Audio qualification worker stopped.")?;
                viewer.take();
                result
            })?;
            println!("{}", serde_json::to_string(&metrics)?);
        }
        Some("--qualify") => {
            let fixture = match std::env::args().nth(2).as_deref() {
                None => sources::fixture(),
                Some("monitor") => sources::enumerate()
                    .into_iter()
                    .find(|source| matches!(source.target, sources::Target::Monitor(_)))
                    .context("No monitor is available.")?,
                Some("monitor-wgc") => {
                    let mut monitor = sources::enumerate()
                        .into_iter()
                        .find(|source| matches!(source.target, sources::Target::Monitor(_)))
                        .context("No monitor is available.")?;
                    if let sources::Target::Monitor(handle) = monitor.target {
                        monitor.target = sources::Target::MonitorWgc(handle);
                    }
                    monitor
                }
                Some(id) => sources::enumerate()
                    .into_iter()
                    .find(|source| source.info.id == id)
                    .context("That capture source is unavailable.")?,
            };
            let encoders = encoding::encoders();
            ensure!(
                !encoders.is_empty(),
                "No NVIDIA/AMD hardware encoders were detected."
            );
            for encoder in encoders {
                let stop = Arc::new(AtomicBool::new(false));
                let encoding =
                    Encoding::start(&fixture, &encoder, Mode::baseline(&fixture)?, &stop)?;
                let metrics = encoding.run(
                    None,
                    stop,
                    Arc::new(Mutex::new(Metrics::default())),
                    Some(120),
                )?;
                println!(
                    "{}",
                    serde_json::json!({"encoder": encoder.factory, "codec": encoder.codec, "metrics": metrics})
                );
            }
        }
        Some("--publish-fixture") => {
            let mut input = String::new();
            std::io::stdin().take(8193).read_to_string(&mut input)?;
            ensure!(input.len() <= 8192, "Fixture input is too large.");
            let request: FixtureRequest =
                serde_json::from_str(&input).context("Invalid fixture input.")?;
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            runtime.block_on(publish_fixture(request))?;
        }
        Some(_) => bail!(
            "Unknown option. Use --probe, --sources, --audio-apps, --qualify, --qualify-audio or --publish-fixture."
        ),
        None => {
            let options = eframe::NativeOptions {
                viewport: eframe::egui::ViewportBuilder::default()
                    .with_inner_size([760.0, 620.0])
                    .with_min_inner_size([600.0, 560.0]),
                renderer: eframe::Renderer::Glow,
                ..Default::default()
            };
            eframe::run_native(
                "FreedomStream",
                options,
                Box::new(|context| {
                    context.egui_ctx.set_visuals(eframe::egui::Visuals::dark());
                    Ok(Box::new(ui::PublisherUi::new()))
                }),
            )
            .map_err(|_| anyhow::anyhow!("Could not open the publisher window."))?;
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FixtureRequest {
    invitation_url: String,
    display_name: String,
    codec: Codec,
    seconds: u64,
    source_id: Option<String>,
    #[serde(default)]
    audio: audio::Choice,
    #[serde(default)]
    exclusions: Vec<String>,
    #[serde(default)]
    reconnect_after_seconds: Option<u64>,
    #[serde(default)]
    linger_seconds: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AudioRequest {
    choice: audio::Choice,
    #[serde(default)]
    exclusions: Vec<String>,
    seconds: u64,
    #[serde(default)]
    viewer_invitation: Option<String>,
}

async fn publish_fixture(request: FixtureRequest) -> Result<()> {
    ensure!(
        (1..=75).contains(&request.seconds),
        "Fixture duration must be between 1 and 75 seconds."
    );
    ensure!(
        request.linger_seconds <= 8
            && request
                .reconnect_after_seconds
                .is_none_or(|delay| delay >= 1 && delay <= request.seconds.saturating_sub(5)),
        "Invalid recovery fixture timing."
    );
    let admission =
        admission::Admission::join(&request.invitation_url, &request.display_name).await?;
    let access = admission.media_access().await?;
    let mut options = RoomOptions::default();
    options.auto_subscribe = false;
    options.join_retries = 0;
    let (room, mut events) = tokio::time::timeout(
        Duration::from_secs(15),
        Room::connect(&access.server_url, &access.token, options),
    )
    .await
    .context("Fixture media connection timed out.")?
    .map_err(|error| {
        anyhow::anyhow!(
            "Fixture media connection failed: {}",
            admission::media_diagnostic(error)
        )
    })?;
    let stop = Arc::new(AtomicBool::new(false));
    let cancel = stop.clone();
    let republished = Arc::new(AtomicU64::new(0));
    let publication_count = republished.clone();
    let events = tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            match event {
                RoomEvent::Disconnected { .. } => cancel.store(true, Ordering::Release),
                RoomEvent::LocalTrackRepublished { .. } => {
                    publication_count.fetch_add(1, Ordering::Relaxed);
                }
                _ => {}
            }
        }
    });
    let encoder = encoding::encoders()
        .into_iter()
        .find(|encoder| encoder.codec == request.codec)
        .context("No hardware encoder supports the requested codec.")?;
    let source = match request.source_id {
        Some(id) => sources::enumerate()
            .into_iter()
            .find(|source| source.info.id == id)
            .context("Fixture capture source is unavailable.")?,
        None => sources::fixture(),
    };
    let result = async {
        let mut active = publisher::Stream::start(&room, source, encoder.clone(), stop, request.audio, request.exclusions).await?;
        active.start_previews(admission.clone());
        let preview_uploads = active.preview_uploads.clone();
        let audio_metrics = active.audio_metrics.clone();
        if let Some(after) = request.reconnect_after_seconds {
            tokio::time::sleep(Duration::from_secs(after)).await;
            room.simulate_scenario(livekit::rtc_engine::SimulateScenario::FullReconnect).await.map_err(|_| anyhow::anyhow!("Could not request a local fixture reconnect."))?;
            tokio::time::sleep(Duration::from_secs(request.seconds - after)).await;
        } else { tokio::time::sleep(Duration::from_secs(request.seconds)).await; }
        let metrics = active.stop(&room).await?;
        let audio = audio_metrics.lock().map(|value| value.clone()).unwrap_or_default();
        println!("{}", serde_json::json!({"encoder": encoder.factory, "codec": encoder.codec, "metrics": metrics, "audio": audio, "captureStopped": true, "republished": republished.load(Ordering::Relaxed), "previewUploads": preview_uploads.load(Ordering::Relaxed)}));
        tokio::time::sleep(Duration::from_secs(request.linger_seconds)).await;
        Ok::<_, anyhow::Error>(())
    }.await;
    let _ = room.close().await;
    events.abort();
    let _ = events.await;
    admission.leave().await;
    result
}
