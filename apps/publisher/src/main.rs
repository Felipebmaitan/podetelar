#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]
mod admission;
mod audio;
mod browser_links;
mod connection;
mod delivery;
mod diagnostics;
mod encoding;
mod locale;
mod network;
mod playback;
mod preview;
mod publisher;
mod quality;
mod settings;
mod sources;
mod ui;
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
        #[cfg(windows)]
        if std::env::args_os().nth(1).is_none() {
            // A normal GUI launch has no stderr console. Keep authority-bearing
            // SDK details out of dialogs while making startup failures visible.
            use windows::{
                Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
                core::w,
            };
            unsafe {
                let _ = MessageBoxW(
                    None,
                    w!(
                        "FreedomStream could not start. Check your graphics driver and app files, then try again."
                    ),
                    w!("FreedomStream"),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
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
            serde_json::json!({"encoders": encoding::encoders(), "decoders": playback::decoder_codecs(), "sourceCount": sources::enumerate().len(), "runtime": gstreamer::version_string().to_string()})
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
        Some("--measure-connection") => {
            // Private invitation is stdin-only; output contains capacity, never authority.
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct Request {
                invitation_url: String,
            }
            let mut input = String::new();
            std::io::stdin().take(4097).read_to_string(&mut input)?;
            ensure!(
                input.len() <= 4096,
                "Connection fixture input is too large."
            );
            let request: Request =
                serde_json::from_str(&input).context("Invalid connection fixture input.")?;
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async {
                let admission=admission::Admission::join(&request.invitation_url,"Connection test").await?;
                let measured=admission.measure_connection().await;
                admission.leave().await;
                println!("{}",serde_json::json!({"downloadBps":measured.download_bps,"uploadBps":measured.upload_bps,"watchingAllowance":measured.watching_allowance(),"uploadVideoCap":measured.upload_video_cap(),"error":measured.error}));
                Ok::<_,anyhow::Error>(())
            })?;
        }
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
                let _playback = if request.playback_tone {
                    Some(playback::qualification_tone()?)
                } else {
                    None
                };
                tokio::task::spawn_blocking(move || {
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
                .context("Audio qualification worker stopped.")?
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
                let mut mode = Mode::baseline(&fixture.info)?;
                if let Some(fps) = std::env::args().nth(3) {
                    mode.fps = fps
                        .parse()
                        .context("Choose a supported qualification frame rate.")?;
                    ensure!(
                        quality::FRAME_RATES.contains(&mode.fps),
                        "Choose a supported frame rate up to 120 fps."
                    );
                }
                let encoding = Encoding::start(&fixture, &encoder, mode, &stop)?;
                let metrics = encoding.run(
                    None,
                    stop,
                    Arc::new(Mutex::new(Metrics::default())),
                    Some(mode.fps as u64 * 3),
                )?;
                println!(
                    "{}",
                    serde_json::json!({"encoder": encoder.factory, "codec": encoder.codec, "mode":mode,"metrics": metrics})
                );
            }
        }
        Some("--watch-fixture") => {
            let mut input = String::new();
            std::io::stdin().take(8193).read_to_string(&mut input)?;
            ensure!(input.len() <= 8192, "Fixture input is too large.");
            let request: WatchRequest =
                serde_json::from_str(&input).context("Invalid watcher input.")?;
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            runtime.block_on(watch_fixture(request))?;
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
        Some(value) if value != "--browser-link" => bail!(
            "Unknown option. Use --probe, --sources, --audio-apps, --qualify, --qualify-audio or --publish-fixture / --watch-fixture."
        ),
        _ => {
            let browser_link = if std::env::args().nth(1).as_deref() == Some("--browser-link") {
                ensure!(
                    std::env::args_os().len() == 3,
                    "Invalid browser invitation."
                );
                Some(
                    std::env::args()
                        .nth(2)
                        .context("Invalid browser invitation.")?,
                )
            } else {
                None
            };
            let options = eframe::NativeOptions {
                viewport: eframe::egui::ViewportBuilder::default()
                    .with_icon(eframe::egui::IconData {
                        rgba: include_bytes!("../../../assets/windows/freedomstream-64.rgba")
                            .to_vec(),
                        width: 64,
                        height: 64,
                    })
                    .with_inner_size([1280.0, 720.0])
                    .with_min_inner_size([960.0, 540.0]),
                renderer: eframe::Renderer::Glow,
                ..Default::default()
            };
            eframe::run_native(
                "FreedomStream",
                options,
                Box::new(move |context| {
                    ui::configure_theme(&context.egui_ctx);
                    Ok(Box::new(ui::PublisherUi::with_browser_link(browser_link)))
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
    #[serde(default)]
    sample_rates: bool,
    #[serde(default)]
    upload_limit_bps: Option<u64>,
    #[serde(default)]
    route: connection::Route,
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
    #[serde(default)]
    quality: quality::Settings,
    #[serde(default)]
    fixture_4k: bool,
    #[serde(default)]
    bitrate_after_seconds: Option<u64>,
    #[serde(default)]
    next_bitrate: Option<u64>,
    #[serde(default)]
    reconfigure_after_seconds: Option<u64>,
    #[serde(default)]
    next_quality: Option<quality::Settings>,
    #[serde(default)]
    next_codec: Option<Codec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AudioRequest {
    choice: audio::Choice,
    #[serde(default)]
    exclusions: Vec<String>,
    seconds: u64,
    #[serde(default)]
    playback_tone: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WatchRequest {
    invitation_url: String,
    seconds: u64,
    #[serde(default)]
    h264_only: bool,
    #[serde(default)]
    wait_for_preview: bool,
    #[serde(default)]
    preview_only: bool,
    #[serde(default)]
    consume_pictures: bool,
    #[serde(default)]
    download_limit_mbps: Option<u64>,
    #[serde(default)]
    reconnect_after_seconds: Option<u64>,
}

async fn watch_fixture(request: WatchRequest) -> Result<()> {
    ensure!(
        (10..=120).contains(&request.seconds),
        "Watcher duration must be 10–120 seconds."
    );
    let admission =
        admission::Admission::join(&request.invitation_url, "Native playback check").await?;
    let access = admission.media_access().await?;
    let (room, mut events) = Room::connect(
        &access.server_url,
        &access.token,
        connection::options(connection::Route::Auto),
    )
    .await
    .map_err(|error| {
        anyhow::anyhow!(
            "Native watcher connection failed: {}",
            admission::media_diagnostic(error)
        )
    })?;
    let mut playback = playback::Playback::new();
    let recovery = playback.recovery_signal();
    let events_recovery = recovery.clone();
    let event_task = tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            if matches!(event, RoomEvent::Reconnecting | RoomEvent::Reconnected) {
                events_recovery.fetch_add(1, Ordering::AcqRel);
            }
        }
    });
    playback.compatibility_only(request.h264_only);
    playback.measure_audio();
    ensure!(
        request
            .download_limit_mbps
            .is_none_or(|mbps| (1..=200).contains(&mbps)),
        "Invalid download allowance."
    );
    playback.set_budget(request.download_limit_mbps.map(|mbps| mbps * 1_000_000));
    let mut samples = Vec::new();
    let started = std::time::Instant::now();
    let mut next_sample = 0;
    let mut watch_after = if request.wait_for_preview {
        None
    } else {
        Some(2)
    };
    let mut preview_before_watching = false;
    let mut reconnected = false;
    let mut received = std::collections::HashMap::<String, RemoteTrack>::new();
    let mut byte_totals = std::collections::HashMap::<String, u64>::new();
    let mut previous_bytes = std::collections::HashMap::<String, u64>::new();
    let mut known = std::collections::HashSet::new();
    let mut previous_watching = false;
    let result: Result<()> = async {
        while started.elapsed().as_secs() < request.seconds {
            let elapsed = started.elapsed().as_secs();
            if !reconnected && request.reconnect_after_seconds.is_some_and(|after| elapsed >= after) {
                reconnected = true;
                room.simulate_scenario(livekit::rtc_engine::SimulateScenario::FullReconnect).await
                    .map_err(|_| anyhow::anyhow!("Could not exercise watcher recovery."))?;
            }
            let tiles = playback.reconcile(&room, &admission).await?;
            if watch_after.is_none() {
                let preview_ready = tiles.iter().any(|tile| !tile.watching && tile.picture.lock().is_ok_and(|picture| picture.is_some()));
                if preview_ready || elapsed >= 8 { watch_after = Some(elapsed); preview_before_watching = preview_ready; }
            }
            let phase = watch_after.filter(|after| elapsed >= *after).map(|after| elapsed - after);
            let watching = !request.preview_only && phase.is_some_and(|phase| !(5..8).contains(&phase));
            for tile in &tiles {
                if watching != previous_watching || (watching && !known.contains(&tile.id)) { playback.watch(tile.id.clone(), watching)?; }
                known.insert(tile.id.clone());
            }
            previous_watching = watching;
            if elapsed >= next_sample {
                let mut decoders = Vec::new();
                for participant in room.remote_participants().values() {
                    for publication in participant.track_publications().values() {
                        if let Some(track) = publication.track() && received.len() < 64 { received.insert(publication.sid().to_string(), track); }
                    }
                }
                for (sid, track) in &received {
                    let stats = match track { RemoteTrack::Video(track) => track.get_stats().await, RemoteTrack::Audio(track) => track.get_stats().await };
                    if let Ok(stats) = stats {
                        for stat in &stats {
                            if let livekit::webrtc::stats::RtcStats::InboundRtp(stat) = stat
                                && !stat.inbound.decoder_implementation.is_empty() {
                                decoders.push(serde_json::json!({"implementation": stat.inbound.decoder_implementation, "hardware": stat.inbound.power_efficient_decoder}));
                            }
                        }
                        let counters: Vec<_> = stats.into_iter().filter_map(|stat| if let livekit::webrtc::stats::RtcStats::InboundRtp(stat) = stat { Some(stat.inbound.bytes_received) } else { None }).collect();
                        if !counters.is_empty() {
                            let bytes = counters.into_iter().sum::<u64>();
                            let delta = previous_bytes.insert(sid.clone(),bytes).map_or(bytes, |last| if bytes >= last { bytes-last } else { bytes });
                            *byte_totals.entry(sid.clone()).or_default() += delta;
                        }
                    }
                }
                samples.push(serde_json::json!({"second": elapsed, "recoveries": recovery.load(Ordering::Acquire), "connected": room.connection_state() == ConnectionState::Connected, "phaseSecond": phase, "watching": watching,"receivedBytes":byte_totals.values().sum::<u64>(), "decoders":decoders, "streams": tiles.iter().map(|tile| serde_json::json!({"codec":tile.codec,"frames":tile.frames.load(Ordering::Relaxed),"watching":tile.watching,"pictureSize":tile.picture.lock().ok().and_then(|p|p.as_ref().map(|image|image.size)),"audio":tile.audio.lock().map(|m|m.clone()).unwrap_or_default()})).collect::<Vec<_>>() }));
                if request.consume_pictures { for tile in &tiles { if let Ok(mut picture) = tile.picture.lock() { picture.take(); } } }
                next_sample = elapsed + 1;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }.await;
    drop(playback);
    let _ = room.close().await;
    event_task.abort();
    let _ = event_task.await;
    admission.leave().await;
    result?;
    println!(
        "{}",
        serde_json::json!({"samples":samples,"closed":true,"previewBeforeWatching":preview_before_watching})
    );
    Ok(())
}

async fn publish_fixture(request: FixtureRequest) -> Result<()> {
    ensure!(
        (1..=900).contains(&request.seconds),
        "Fixture duration must be between 1 and 900 seconds."
    );
    ensure!(
        request.linger_seconds <= 8
            && request
                .reconnect_after_seconds
                .is_none_or(|delay| delay >= 1 && delay <= request.seconds.saturating_sub(5)),
        "Invalid recovery fixture timing."
    );
    ensure!(
        [
            request.bitrate_after_seconds.is_some(),
            request.reconfigure_after_seconds.is_some(),
            request.reconnect_after_seconds.is_some()
        ]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
            <= 1
            && request.bitrate_after_seconds.is_some() == request.next_bitrate.is_some()
            && request.reconfigure_after_seconds.is_some() == request.next_quality.is_some(),
        "Choose one complete fixture transition."
    );
    let admission =
        admission::Admission::join(&request.invitation_url, &request.display_name).await?;
    let access = admission.media_access().await?;
    let options = connection::options(request.route);
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
    let cancellation = Arc::new(publisher::CaptureCancellation::default());
    let stop = cancellation.register();
    let cancel = cancellation.clone();
    let republished = Arc::new(AtomicU64::new(0));
    let publication_count = republished.clone();
    let events = tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            match event {
                RoomEvent::Disconnected { .. } => cancel.cancel(),
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
        None => {
            let mut source = sources::fixture();
            if request.fixture_4k {
                source.info.width = 3840;
                source.info.height = 2160;
            }
            source
        }
    };
    let result = async {
        let config=publisher::StreamConfig { source,encoder:encoder.clone(),quality:request.quality,audio:request.audio,exclusions:request.exclusions };
        ensure!(request.upload_limit_bps.is_none_or(|v|(64_000..=100_000_000).contains(&v)),"Invalid fixture upload ceiling.");
        let mut active = if request.upload_limit_bps.is_some() {
            publisher::Stream::start_with_upload_cap(&room, config.clone(), stop, request.upload_limit_bps).await?
        } else {
            publisher::Stream::start(&room, config.clone(), stop).await?
        };
        let initial_sid=active.sid();
        println!("{}", serde_json::json!({"published":true}));
        active.start_previews(admission.clone());


        if let (Some(after),Some(bitrate))=(request.bitrate_after_seconds,request.next_bitrate) {
            ensure!(after>=1 && after<request.seconds,"Invalid bitrate fixture timing.");
            tokio::time::sleep(Duration::from_secs(after)).await;
            let mut next=config.clone(); next.quality.bitrate=quality::Bitrate::Manual(bitrate);
            ensure!(active.apply_live(&next)?,"Bitrate fixture unexpectedly required a restart.");
            tokio::time::sleep(Duration::from_secs(request.seconds-after)).await;
        } else if let (Some(after),Some(quality))=(request.reconfigure_after_seconds,request.next_quality) {
            ensure!(after>=1 && after<request.seconds,"Invalid reconfiguration fixture timing.");
            tokio::time::sleep(Duration::from_secs(after)).await;
            let mut next=config.clone(); next.quality=quality;
            if let Some(codec)=request.next_codec { next.encoder=encoding::encoders().into_iter().find(|encoder|encoder.codec==codec).context("No requested compatibility encoder.")?; }
            next.source.info.id="fixture:switched".into();
            next.mode()?;
            ensure!(!active.apply_live(&next)?,"Source/codec/mode fixture unexpectedly avoided a restart.");
            active.stop(&room).await?;
            active=publisher::Stream::start_with_upload_cap(&room,next,cancellation.register(),request.upload_limit_bps).await?;
            active.start_previews(admission.clone());

            tokio::time::sleep(Duration::from_secs(request.seconds-after)).await;
        } else if let Some(after) = request.reconnect_after_seconds {
            tokio::time::sleep(Duration::from_secs(after)).await;
            room.simulate_scenario(livekit::rtc_engine::SimulateScenario::FullReconnect).await.map_err(|_| anyhow::anyhow!("Could not request a local fixture reconnect."))?;
            tokio::time::sleep(Duration::from_secs(request.seconds - after)).await;
        } else { active = serve_fixture(&room, &admission, active, request.seconds, cancellation.clone(), request.sample_rates).await?; }
        let preview_uploads = active.preview_uploads.clone(); let audio_metrics = active.audio_metrics.clone(); let delivery = active.delivery.clone();
        let mode=active.mode;
        let encoder=active.encoder().clone();
        let final_sid=active.sid();
        let delivered=delivery.lock().map(|value|value.clone()).unwrap_or_default();
        let metrics = active.stop(&room).await?;
        let audio = audio_metrics.lock().map(|value| value.clone()).unwrap_or_default();
        println!("{}", serde_json::json!({"encoder": encoder.factory, "codec": encoder.codec, "initialSid":initial_sid,"finalSid":final_sid,"mode":mode,"delivery":delivered,"metrics": metrics, "audio": audio, "captureStopped": true, "republished": republished.load(Ordering::Relaxed), "previewUploads": preview_uploads.load(Ordering::Relaxed)}));
        tokio::time::sleep(Duration::from_secs(request.linger_seconds)).await;
        Ok::<_, anyhow::Error>(())
    }.await;
    let _ = room.close().await;
    events.abort();
    let _ = events.await;
    admission.leave().await;
    result
}

async fn serve_fixture(
    room: &Room,
    admission: &admission::Admission,
    mut active: publisher::Stream,
    seconds: u64,
    cancellation: Arc<publisher::CaptureCancellation>,
    sample_rates: bool,
) -> Result<publisher::Stream> {
    let started = std::time::Instant::now();
    let mut next_sample = 0;
    while started.elapsed().as_secs() < seconds {
        active.constrain_watchers(room);
        let second = started.elapsed().as_secs();
        if sample_rates && second >= next_sample {
            let metrics = active.metrics.lock().map(|m| m.clone()).unwrap_or_default();
            println!(
                "{}",
                serde_json::json!({"rateSample":true,"second":second,"encoderBps":metrics.encoder_bitrate,"fps":metrics.fps,"watcherCap":playback::requested_cap(room)})
            );
            next_sample = second + 1;
        }
        let stop = cancellation.register();
        if active.cancelled() {
            break;
        }
        if active.encoder().codec != Codec::H264 && playback::needs_h264(room) {
            let config = active.fallback_config()?;
            active = active
                .fallback(room, admission.clone(), stop, config)
                .await?;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Ok(active)
}
