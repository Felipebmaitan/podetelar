#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]
mod account;
mod activation;
#[path = "../../activation_protocol.rs"]
mod activation_protocol;
mod admission;
mod audio;
mod av_qualification;
mod browser_links;
mod codec_policy;
mod connection;
mod delivery;
mod device;
mod diagnostics;
#[cfg(feature = "media-qualification")]
mod encoded_fixture;
mod encoding;
mod following;
mod locale;
#[cfg(target_os = "macos")]
mod macos;
mod native_output;
mod network;
#[cfg(feature = "performance-experiments")]
#[allow(dead_code)]
mod planar_experiment;
mod playback;
mod playback_qualification;
mod preview;
mod profile;
mod publisher;
mod quality;
#[cfg(feature = "performance-experiments")]
#[allow(dead_code)]
mod resource_experiment;
mod settings;
mod source_picker;
mod sources;
mod stream_diagnostics;
mod theme;
mod ui;
mod ui_style;
#[path = "../../update_protocol.rs"]
mod update_protocol;
mod updates;
mod video_image;
#[cfg(windows)]
mod win32;
mod worker;

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
        eprintln!("PodeTelar: {error}");
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
                        "PodeTelar could not start. Check your graphics driver and app files, then try again."
                    ),
                    w!("PodeTelar"),
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
    if std::env::var("PODETELAR_DIAGNOSTICS").as_deref() == Ok("1") {
        diagnostics::enable();
    }
    gstreamer::init()
        .context("GStreamer could not initialize. Check the native runtime installation.")?;
    match std::env::args().nth(1).as_deref() {
        Some("--profile-fixture") => {
            let mut input = String::new();
            std::io::stdin().take(8193).read_to_string(&mut input)?;
            ensure!(input.len() <= 8192, "Profile fixture input is too large.");
            let request = serde_json::from_str(&input).context("Invalid profile fixture input.")?;
            return tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?
                .block_on(profile::qualify(request));
        }
        #[cfg(feature = "media-qualification")]
        Some("--publish-av1-fixture") => {
            let mut input = String::new();
            std::io::stdin().take(8193).read_to_string(&mut input)?;
            ensure!(input.len() <= 8192, "Encoded fixture input is too large.");
            let request = serde_json::from_str(&input).context("Invalid encoded fixture input.")?;
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?
                .block_on(encoded_fixture::publish(request))?;
        }
        Some("--av-timing-source") => av_qualification::source(
            std::env::args()
                .nth(2)
                .map(|s| s.parse::<u64>())
                .transpose()?
                .unwrap_or(60),
        )?,
        Some("--qualify-playback") => {
            let legacy = match std::env::args().nth(2).as_deref() {
                None => false,
                Some("legacy") => true,
                _ => bail!("Use --qualify-playback [legacy]."),
            };
            playback_qualification::run(legacy)?;
        }
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
        Some("--cameras") => println!(
            "{}",
            serde_json::to_string(
                &sources::camera::enumerate()
                    .into_iter()
                    .map(|device| device.info)
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
            let rate = std::env::args()
                .nth(3)
                .map(|rate| rate.parse::<u32>())
                .transpose()?
                .unwrap_or(48_000);
            ensure!(
                [44_100, 48_000, 96_000, 192_000].contains(&rate),
                "Use a supported test sample rate."
            );
            let samples = rate / 100;
            let sink = if cfg!(target_os = "macos") {
                "osxaudiosink"
            } else {
                "wasapi2sink low-latency=true"
            };
            let sink = if rate == 48_000 {
                sink.to_owned()
            } else {
                format!("audioconvert ! audioresample ! {sink}")
            };
            let description = if frequency == 440 {
                format!(
                    "audiomixer name=mix ! audioconvert ! audio/x-raw,rate={rate},channels=2 ! {sink} audiotestsrc is-live=true freq=440 volume=0.02 samplesperbuffer={samples} ! audio/x-raw,format=F32LE,rate={rate},channels=1 ! audiopanorama panorama=-1 ! mix. audiotestsrc is-live=true freq=660 volume=0.02 samplesperbuffer={samples} ! audio/x-raw,format=F32LE,rate={rate},channels=1 ! audiopanorama panorama=1 ! mix."
                )
            } else {
                format!(
                    "audiotestsrc is-live=true freq={frequency} volume=0.02 samplesperbuffer={samples} ! audio/x-raw,rate={rate},channels=2 ! {sink}"
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
                "No supported hardware encoders were detected."
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
        Some("--qualify-dual-sharing") => {
            let mut input = String::new();
            std::io::stdin().take(8193).read_to_string(&mut input)?;
            ensure!(input.len() <= 8192, "Fixture input is too large.");
            let request = serde_json::from_str(&input).context("Invalid dual-sharing fixture.")?;
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?
                .block_on(qualify_dual_sharing(request))?;
        }
        Some(value) if value != "--browser-link" => bail!(
            "Unknown option. Use --probe, --sources, --cameras, --audio-apps, --qualify, --qualify-audio or --publish-fixture / --watch-fixture."
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
                        rgba: include_bytes!("../../../assets/windows/podetelar-64.rgba").to_vec(),
                        width: 64,
                        height: 64,
                    })
                    .with_inner_size([1280.0, 720.0])
                    .with_min_inner_size([960.0, 540.0]),
                #[cfg(target_os = "macos")]
                event_loop_builder: Some(Box::new(|builder| {
                    use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
                    builder.with_activation_policy(ActivationPolicy::Regular);
                })),
                renderer: eframe::Renderer::Glow,
                ..Default::default()
            };
            eframe::run_native(
                "PodeTelar",
                options,
                Box::new(move |context| {
                    #[cfg(target_os = "macos")]
                    macos::request_capture_permission();
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
    follow_stream_policy: bool,
    #[serde(default)]
    source_audio: bool,
    #[serde(default)]
    upload_limit_bps: Option<u64>,
    #[serde(default)]
    route: connection::Route,
    invitation_url: String,
    display_name: String,
    codec: Codec,
    seconds: u64,
    source_id: Option<String>,
    camera_id: Option<String>,
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DualSharingRequest {
    invitation_url: String,
    source_id: String,
    #[serde(default)]
    camera_id: String,
    camera_codec: Option<Codec>,
    #[serde(default)]
    check_live_policy: bool,
    #[serde(default)]
    check_camera_view: bool,
    camera_policy_bitrates: Option<[u64; 2]>,
}

async fn qualify_dual_sharing(request: DualSharingRequest) -> Result<()> {
    use publisher::{Command, Controller, Snapshot};
    ensure!(
        admission::loopback_url(&request.invitation_url)
            && request.source_id.starts_with("window:"),
        "Dual-sharing qualification requires an isolated local room and an explicit window."
    );
    async fn wait(controller: &Controller, ready: impl Fn(&Snapshot) -> bool) -> Result<Snapshot> {
        tokio::time::timeout(Duration::from_secs(25), async {
            loop {
                let state = controller.snapshot();
                if ready(&state) {
                    return Ok(state);
                }
                ensure!(
                    state.error.is_empty() || state.busy,
                    "Dual-sharing operation failed: {}",
                    state.error
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .context("Dual-sharing operation did not finish.")?
    }
    let controller = Controller::for_local_media_qualification();
    controller.send(Command::Join {
        invitation: request.invitation_url,
        name: "Dual publisher".into(),
        route: connection::Route::Auto,
        account: None,
        voice: None,
    });
    let joined = wait(&controller, |state| state.connection == "connected").await?;
    let encoder = joined
        .encoders
        .iter()
        .find(|encoder| encoder.codec == Codec::H264)
        .context("No H.264 hardware encoder is available.")?;
    controller.send(Command::Start {
        source_id: request.source_id,
        encoder: encoder.factory.clone(),
        quality: quality::Settings {
            fps: 30,
            ..Default::default()
        },
        audio: audio::Choice::Off,
        exclusions: Vec::new(),
    });
    wait(&controller, |state| state.sharing).await?;
    let start_camera = || {
        controller.send(Command::StartCamera {
            device_id: request.camera_id.clone(),
            preferred_codec: request.camera_codec,
            preview_release: None,
            quality: quality::Settings {
                resolution: if request.check_live_policy {
                    quality::Resolution::P1080
                } else {
                    quality::Resolution::P720
                },
                fps: 30,
                bitrate: if request.camera_policy_bitrates.is_some() {
                    quality::Bitrate::Auto
                } else {
                    quality::Bitrate::Manual(2_000_000)
                },
                ..Default::default()
            },
        })
    };
    start_camera();
    let initial = wait(&controller, |state| state.sharing && state.camera_sharing).await?;
    let stop_camera_at = tokio::time::Instant::now() + Duration::from_secs(17);
    println!("{}", serde_json::json!({"both":true}));
    if request.check_camera_view {
        let feed = initial
            .camera_feed
            .as_ref()
            .context("Camera view is unavailable.")?;
        ensure!(
            feed.next_sample().is_none(),
            "Camera view was active before it was requested."
        );
        let start = std::time::Instant::now();
        let mut frames = 0;
        while start.elapsed() < Duration::from_secs(1) {
            if let Some(sample) = feed.next_sample() {
                let picture =
                    video_image::camera_image(&sample).context("Invalid camera view frame.")?;
                ensure!(picture.size == [640, 360], "Wrong camera view dimensions.");
                frames += 1;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        ensure!(
            frames >= 10,
            "Camera view only produced {frames} frames in one second."
        );
    }
    if let Some(expected) = request.camera_policy_bitrates {
        for bitrate in expected {
            wait(&controller, |state| {
                state.sharing
                    && state.camera_sharing
                    && state.mode == initial.mode
                    && state
                        .camera_mode
                        .is_some_and(|mode| mode.bitrate == bitrate)
            })
            .await?;
            println!("{}", serde_json::json!({"cameraBitrate":bitrate}));
        }
        tokio::time::sleep(Duration::from_secs(4)).await;
        let state = controller.snapshot();
        ensure!(
            state.mode == initial.mode && state.camera_sharing,
            "Concurrent publication did not stay active."
        );
        controller.send(Command::Leave);
        wait(&controller, |state| {
            !state.any_sharing() && state.connection == "disconnected"
        })
        .await?;
        println!(
            "{}",
            serde_json::json!({"screenMode":state.mode,"cameraMode":state.camera_mode})
        );
        return Ok(());
    }
    if request.check_live_policy {
        let meets_cap = |mode: Option<quality::Mode>| {
            mode.is_some_and(|mode| mode.fps == 24 && mode.bitrate == 1_000_000)
        };
        let state = wait(&controller, |state| {
            state.sharing
                && state.camera_sharing
                && meets_cap(state.mode)
                && meets_cap(state.camera_mode)
        })
        .await?;
        // Allow the receiver to decode both replacement publications under the new cap.
        tokio::time::sleep(Duration::from_secs(8)).await;
        controller.send(Command::Leave);
        wait(&controller, |state| {
            !state.any_sharing() && state.connection == "disconnected"
        })
        .await?;
        println!(
            "{}",
            serde_json::json!({"screenMode": state.mode, "cameraMode": state.camera_mode})
        );
        return Ok(());
    }
    tokio::time::sleep_until(stop_camera_at).await;
    controller.send(Command::StopCamera);
    wait(&controller, |state| state.sharing && !state.camera_sharing).await?;
    tokio::time::sleep(Duration::from_secs(4)).await;
    start_camera();
    wait(&controller, |state| state.sharing && state.camera_sharing).await?;
    tokio::time::sleep(Duration::from_secs(4)).await;
    controller.send(Command::Stop);
    wait(&controller, |state| !state.sharing && state.camera_sharing).await?;
    tokio::time::sleep(Duration::from_secs(4)).await;
    controller.send(Command::Leave);
    wait(&controller, |state| {
        !state.any_sharing() && state.connection == "disconnected"
    })
    .await?;
    println!("{}", serde_json::json!({"closed":true}));
    Ok(())
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
    #[serde(default)]
    soak: bool,
    #[serde(default)]
    output_pause_after_seconds: Option<u64>,
    #[serde(default)]
    output_switch_after_seconds: Option<u64>,
    #[serde(default)]
    fail_direct_output: bool,
    #[serde(default)]
    continuous_watch: bool,
    #[serde(default)]
    watch_once_per_publisher: bool,
    #[serde(default)]
    watch_schedule: Vec<FixtureWatch>,
    #[serde(default)]
    measure_output_audio: bool,
    #[serde(default)]
    volume_schedule: Vec<FixtureVolume>,
    #[serde(default)]
    audiovisual_timing: bool,
    #[serde(default)]
    diagnostics: bool,
    invitation_url: String,
    seconds: u64,
    #[serde(default)]
    h264_only: bool,
    #[serde(default)]
    hevc_only: bool,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FixtureVolume {
    second: u64,
    percent: u32,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FixtureWatch {
    second: u64,
    enabled: bool,
    display_name: String,
}

fn fixture_sync_groups(room: &Room) -> serde_json::Value {
    let mut groups = Vec::new();
    let mut mismatched = 0;
    for participant in room.remote_participants().values() {
        let tracks = participant.track_publications();
        let video = tracks
            .values()
            .find(|p| p.source() == TrackSource::Screenshare)
            .and_then(|p| p.track());
        let audio = tracks
            .values()
            .find(|p| p.source() == TrackSource::ScreenshareAudio)
            .and_then(|p| p.track());
        if let (Some(RemoteTrack::Video(video)), Some(RemoteTrack::Audio(audio))) = (video, audio) {
            let audio_ids = audio.sync_stream_ids();
            if let Some(group) = video
                .sync_stream_ids()
                .into_iter()
                .find(|id| audio_ids.contains(id))
            {
                groups.push(group);
            } else {
                mismatched += 1;
            }
        }
    }
    let distinct = groups
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    serde_json::json!({"pairedPublishers": groups.len(), "mismatchedPublishers": mismatched, "distinctGroups":distinct})
}

async fn watch_fixture(request: WatchRequest) -> Result<()> {
    ensure!(
        request.watch_schedule.len() <= 32
            && request
                .watch_schedule
                .iter()
                .all(|step| step.second <= request.seconds),
        "Invalid fixture watch schedule."
    );
    ensure!(
        request.volume_schedule.len() <= 32
            && request
                .volume_schedule
                .iter()
                .all(|step| step.percent <= 100 && step.second <= request.seconds),
        "Invalid fixture volume schedule."
    );
    ensure!(
        (10..=if request.soak { 2000 } else { 120 }).contains(&request.seconds)
            && (!request.soak || request.audiovisual_timing && request.continuous_watch),
        "Invalid watcher qualification duration."
    );
    let admission =
        admission::Admission::join(&request.invitation_url, "Native playback check").await?;
    let mut playback = playback::Playback::new();
    playback.compatibility_only(request.h264_only);
    if request.hevc_only {
        playback.disable_av1();
    }
    let access = admission
        .media_access_with_decoders(playback.decoders())
        .await?;
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
    let recovery = playback.recovery_signal();
    let events_recovery = recovery.clone();
    let event_task = tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            if matches!(event, RoomEvent::Reconnecting | RoomEvent::Reconnected) {
                events_recovery.fetch_add(1, Ordering::AcqRel);
            }
        }
    });
    playback.measure_audio();
    let timing = (request.audiovisual_timing || request.measure_output_audio)
        .then(av_qualification::Events::new);
    if let Some(timing) = &timing {
        playback.measure_av(timing.clone());
    }
    let observer = if let Some(timing) = &timing {
        Some(av_qualification::Observer::start(timing.clone())?)
    } else {
        None
    };
    ensure!(
        request
            .download_limit_mbps
            .is_none_or(|mbps| (1..=200).contains(&mbps)),
        "Invalid download allowance."
    );
    playback.set_budget(request.download_limit_mbps.map(|mbps| mbps * 1_000_000));
    let mut samples = Vec::new();
    let started_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    let started = std::time::Instant::now();
    let mut next_sample = 0;
    let mut requested_watches = std::collections::HashSet::new();
    let mut watch_after = if request.wait_for_preview && !request.watch_once_per_publisher {
        None
    } else {
        Some(2)
    };
    let mut preview_before_watching = false;
    let mut reconnected = false;
    let mut output_paused = false;
    let mut output_switched = false;
    let mut output_failure_applied = false;
    let mut alternate_output_available = false;
    let mut received = std::collections::HashMap::<String, RemoteTrack>::new();
    let mut byte_totals = std::collections::HashMap::<String, u64>::new();
    let mut previous_bytes = std::collections::HashMap::<String, u64>::new();
    let result: Result<()> = async {
        while started.elapsed().as_secs() < request.seconds {
            let elapsed = started.elapsed().as_secs();
            if !reconnected && request.reconnect_after_seconds.is_some_and(|after| elapsed >= after) {
                reconnected = true;
                room.simulate_scenario(livekit::rtc_engine::SimulateScenario::FullReconnect).await
                    .map_err(|_| anyhow::anyhow!("Could not exercise watcher recovery."))?;
            }
            if !output_paused && request.output_pause_after_seconds.is_some_and(|second| elapsed >= second) {
                output_paused = native_output::qualification_pause(3);
            }
            if request.fail_direct_output && !output_failure_applied {
                output_failure_applied = native_output::qualification_fail_platform_start();
            }
            if !output_switched && request.output_switch_after_seconds.is_some_and(|second| elapsed >= second) {
                alternate_output_available = native_output::qualification_switch(3);
                output_switched = true;
            }
            let tiles = playback.reconcile(&room, &admission).await?;
            ensure!(!request.audiovisual_timing || tiles.len() <= 1, "A/V timing qualification requires one isolated publisher.");
            if watch_after.is_none() {
                let preview_ready = tiles.iter().any(|tile| !tile.watching && tile.picture.lock().is_ok_and(|picture| picture.is_some()));
                if preview_ready || elapsed >= 8 { watch_after = Some(elapsed); preview_before_watching = preview_ready; }
            }
            let phase = watch_after.filter(|after| elapsed >= *after).map(|after| elapsed - after);
            let watching = !request.preview_only && phase.is_some_and(|phase| request.continuous_watch || !(5..8).contains(&phase));
            for tile in &tiles {
                if tile.watching { tile.presentation.request(); }
                if request.diagnostics { tile.presentation.diagnostics.open.store(tile.watching, Ordering::Relaxed); }
                if tile.kind == preview::Kind::Camera { continue; }
                let tile_watching = request.watch_schedule.iter().filter(|step| step.second <= elapsed && step.display_name == tile.name).max_by_key(|step| step.second).map_or(watching, |step| step.enabled);
                if tile_watching != tile.watching
                    && (!request.watch_once_per_publisher || tile_watching && requested_watches.insert(tile.id.clone()))
                { playback.watch(tile.id.clone(), tile.kind, tile_watching)?; }
                let requested_volume = request.volume_schedule.iter().filter(|step| step.second <= elapsed && step.display_name.as_ref().is_none_or(|name| name == &tile.name)).max_by_key(|step| step.second).map_or(100, |step| step.percent);
                playback.set_volume(&tile.id, requested_volume);
            }
            if elapsed >= next_sample {
                let mut decoders = Vec::new();
                let mut media_timing = Vec::new();
                for participant in room.remote_participants().values() {
                    for publication in participant.track_publications().values() {
                        if let Some(track) = publication.track() && received.len() < 64 { received.insert(publication.sid().to_string(), track); }
                    }
                }
                for (sid, track) in &received {
                    let stats = match track { RemoteTrack::Video(track) => track.get_stats().await, RemoteTrack::Audio(track) => track.get_stats().await };
                    if let Ok(stats) = stats {
                        for stat in &stats {
                            if let livekit::webrtc::stats::RtcStats::InboundRtp(stat) = stat {
                                media_timing.push(serde_json::json!({
                                    "kind": stat.stream.kind,
                                    "bytes": stat.inbound.bytes_received,
                                    "packets": stat.received.packets_received,
                                    "lost": stat.received.packets_lost,
                                    "jitterSeconds": stat.received.jitter,
                                    "jitterBufferDelaySeconds": stat.inbound.jitter_buffer_delay,
                                    "jitterBufferEmitted": stat.inbound.jitter_buffer_emitted_count,
                                    "decodeSeconds": stat.inbound.total_decode_time,
                                    "decodedFrames": stat.inbound.frames_decoded,
                                }));
                            }
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
                samples.push(serde_json::json!({"second": elapsed, "nativeAudio":native_output::audit(),"syncGroups":fixture_sync_groups(&room), "mediaTiming":media_timing, "recoveries": recovery.load(Ordering::Acquire), "connected": room.connection_state() == ConnectionState::Connected, "phaseSecond": phase, "watching": watching,"receivedBytes":byte_totals.values().sum::<u64>(), "decoders":decoders, "streams": tiles.iter().map(|tile| serde_json::json!({"kind":tile.kind.path(),"diagnosticRates":tile.presentation.diagnostics.rates.lock().ok().and_then(|v|*v),"diagnosticAttempts":tile.presentation.diagnostics.attempts.load(Ordering::Relaxed),"diagnosticSuccesses":tile.presentation.diagnostics.successes.load(Ordering::Relaxed),"diagnosticSender":tile.presentation.diagnostics.sender.lock().ok().and_then(|value|value.as_ref().map(|(_,value)|value.clone())),"status":tile.status,"codec":tile.codec,"frames":tile.frames.load(Ordering::Relaxed),"watching":tile.watching,"pictureSize":tile.picture.lock().ok().and_then(|p|p.as_ref().map(|image|image.size)),"audio":tile.audio.lock().map(|m|m.clone()).unwrap_or_default()})).collect::<Vec<_>>() }));
                if request.consume_pictures { for tile in &tiles { if let Ok(mut picture) = tile.picture.lock() { picture.take(); } } }
                if request.soak {
                    eprintln!("soak-progress second={} video={} audio={} rendering={}",elapsed,
                        tiles.iter().map(|t|t.frames.load(Ordering::Relaxed)).sum::<u64>(),
                        tiles.iter().map(|t|t.audio.lock().map_or(0,|m|m.frames)).sum::<u64>(),
                        native_output::audit()["rendering"].as_bool().unwrap_or(false));
                }
                next_sample = elapsed + if request.soak { 30 } else { 1 };
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
        serde_json::json!({"samples":samples,"startedAtMs":started_at_ms,"closed":true,"outputPauseApplied":output_paused,"alternateOutputAvailable":alternate_output_available,"nativeAudioAfterClose":native_output::audit(),"previewBeforeWatching":preview_before_watching,"audiovisualTiming":timing.as_ref().map(|timing|timing.report())})
    );
    drop(observer);
    Ok(())
}

async fn publish_fixture(request: FixtureRequest) -> Result<()> {
    if request.camera_id.is_some() {
        ensure!(
            admission::loopback_url(&request.invitation_url)
                && request.source_id.is_none()
                && request.seconds <= 120
                && request.reconfigure_after_seconds.is_none()
                && request.bitrate_after_seconds.is_none(),
            "Camera qualification requires an explicit device and an isolated local room."
        );
    }
    ensure!(
        (1..=2100).contains(&request.seconds),
        "Fixture duration must be between 1 and 2100 seconds."
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
    let camera = request
        .camera_id
        .as_ref()
        .map(|id| {
            sources::camera::enumerate()
                .into_iter()
                .find(|device| device.info.id == *id)
                .context("Fixture camera is unavailable.")?
                .source(request.quality)
        })
        .transpose()?;
    let admission =
        admission::Admission::join(&request.invitation_url, &request.display_name).await?;
    let access = admission.media_access().await?;
    if request.camera_id.is_some() && !admission::loopback_url(&access.server_url) {
        admission.leave().await;
        bail!("Camera qualification requires local media signaling.");
    }
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
    // The publisher fixture is a room member even when it does not watch.
    let mut capabilities = playback::Playback::new();
    capabilities.reconcile(&room, &admission).await?;
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
    let source = match (request.source_id, camera) {
        (_, Some(source)) => source,
        (Some(id), None) => sources::enumerate()
            .into_iter()
            .find(|source| source.info.id == id)
            .context("Fixture capture source is unavailable.")?,
        (None, None) => {
            let mut source = sources::fixture();
            if request.fixture_4k {
                source.info.width = 3840;
                source.info.height = 2160;
            }
            source
        }
    };
    let result = async {
        let audio = if request.source_audio { audio::for_source(&source)? } else {request.audio};
        let config=publisher::StreamConfig { source,encoder:encoder.clone(),quality:request.quality,policy:admission.stream_policy().await?,audio,exclusions:request.exclusions };
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
        } else { active = serve_fixture(&room, &admission, active, request.seconds, cancellation.clone(), request.sample_rates, request.follow_stream_policy).await?; }
        let preview_uploads = active.preview_uploads.clone(); let audio_metrics = active.audio_metrics.clone(); let delivery = active.delivery.clone();
        let preview_captures = active.preview_captures();
        let mode=active.mode;
        let encoder=active.encoder().clone();
        let final_sid=active.sid();
        let delivered=delivery.lock().map(|value|value.clone()).unwrap_or_default();
        let server_ended_video = active.video_muted();
        let metrics = if server_ended_video {
            active.stop_ended_publication(&room).await?
        } else {
            active.stop(&room).await?
        };
        let audio = audio_metrics.lock().map(|value| value.clone()).unwrap_or_default();
        println!("{}", serde_json::json!({"encoder": encoder.factory, "codec": encoder.codec, "initialSid":initial_sid,"finalSid":final_sid,"mode":mode,"delivery":delivered,"metrics": metrics, "audio": audio, "captureStopped": true,"serverEndedVideo":server_ended_video,"roomConnectedAfterStop":room.connection_state()==ConnectionState::Connected, "republished": republished.load(Ordering::Relaxed), "previewUploads": preview_uploads.load(Ordering::Relaxed), "previewCaptures": preview_captures}));
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
    follow_stream_policy: bool,
) -> Result<publisher::Stream> {
    let started = std::time::Instant::now();
    let mut next_sample = 0;
    let mut next_policy = 2;
    while started.elapsed().as_secs() < seconds {
        active.constrain_watchers(room);
        let second = started.elapsed().as_secs();
        if sample_rates && second >= next_sample {
            let metrics = active.metrics.lock().map(|m| m.clone()).unwrap_or_default();
            println!(
                "{}",
                serde_json::json!({"rateSample":true,"second":second,"encoderBps":metrics.encoder_bitrate,"fps":metrics.fps,"watcherCap":playback::requested_cap(room, active.kind())})
            );
            next_sample = second + 1;
        }
        let stop = cancellation.register();
        if active.cancelled() || active.video_muted() {
            break;
        }
        if follow_stream_policy && second >= next_policy {
            next_policy = second + 2;
            let policy = admission.stream_policy().await?;
            if active.policy() != &policy {
                (active, _) = active
                    .update_policy(room, admission.clone(), stop, policy)
                    .await?;
                continue;
            }
        }
        if let Some(config) = active.compatible_config(room)? {
            active = active
                .fallback(room, admission.clone(), stop, config)
                .await?;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Ok(active)
}
