mod admission;
mod diagnostics;
mod encoding;
mod publisher;
mod sources;
mod ui;

use anyhow::{Context, Result, bail, ensure};
use encoding::{Codec, Encoding, Metrics, Mode};
use livekit::prelude::*;
use serde::Deserialize;
use std::{
    io::Read,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
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
        Some(_) => bail!("Unknown option. Use --probe, --sources, --qualify or --publish-fixture."),
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
}

async fn publish_fixture(request: FixtureRequest) -> Result<()> {
    ensure!(
        (1..=45).contains(&request.seconds),
        "Fixture duration must be between 1 and 45 seconds."
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
    let events = tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            if matches!(event, RoomEvent::Disconnected { .. }) {
                cancel.store(true, Ordering::Release);
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
        let active = publisher::Stream::start(&room, source, encoder.clone(), stop).await?;
        tokio::time::sleep(Duration::from_secs(request.seconds)).await;
        let metrics = active.stop(&room).await?;
        println!("{}", serde_json::json!({"encoder": encoder.factory, "codec": encoder.codec, "metrics": metrics, "captureStopped": true}));
        Ok::<_, anyhow::Error>(())
    }.await;
    let _ = room.close().await;
    events.abort();
    let _ = events.await;
    admission.leave().await;
    result
}
