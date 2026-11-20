//! Real controller commands with a synthetic-only capture source in a local test room.
use crate::{
    admission, audio, connection, protection,
    publisher::{Command, Controller, Snapshot},
    quality::{Bitrate, Codec, Resolution, Settings},
};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    io::Read,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

static SYNTHETIC_ONLY: AtomicBool = AtomicBool::new(false);

pub(crate) fn synthetic_only() -> bool {
    SYNTHETIC_ONLY.load(Ordering::Acquire)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    invitation_url: String,
}

fn wait(
    controller: &Controller,
    failure: &'static str,
    mut condition: impl FnMut(&Snapshot) -> Result<bool>,
) -> Result<Snapshot> {
    let deadline = Instant::now() + Duration::from_secs(35);
    loop {
        let state = controller.snapshot();
        if condition(&state)? {
            return Ok(state);
        }
        ensure!(Instant::now() < deadline, failure);
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn descriptor(state: &Snapshot) -> Result<Option<String>> {
    state
        .profile_access
        .as_ref()
        .and_then(|admission| admission.protection.as_ref())
        .map(|session| session.descriptor())
        .transpose()
}

fn watch_frames(state: &Snapshot, id: &str) -> Option<u64> {
    state
        .tiles
        .iter()
        .find(|tile| tile.id == id && tile.watching)
        .map(|tile| tile.frames.load(Ordering::Relaxed))
}

fn exercise(controller: &Controller, invitation: String) -> Result<()> {
    controller.send(Command::Join {
        invitation,
        name: "Controller recovery check".into(),
        route: connection::Route::Auto,
        account: None,
        voice: None,
    });
    let joined = wait(controller, "Protected controller join timed out.", |s| {
        Ok(!s.busy && s.connection == "connected" && s.protected)
    })?;
    let original = descriptor(&joined)?.context("Missing original session descriptor.")?;
    let encoder = joined
        .encoders
        .iter()
        .find(|encoder| encoder.codec == Codec::H264)
        .context("The recovery fixture needs a hardware H.264 encoder.")?;
    controller.send(Command::Start {
        source_id: "fixture".into(),
        encoder: encoder.factory.clone(),
        audio: audio::Choice::Off,
        exclusions: vec![],
        quality: Settings {
            resolution: Resolution::P720,
            fps: 30,
            bitrate: Bitrate::Manual(4_000_000),
            ..Settings::default()
        },
    });
    let publishing = wait(controller, "Controller capture did not start.", |s| {
        Ok(!s.busy && s.sharing && s.metrics.frames > 5 && !s.tiles.is_empty())
    })?;
    let id = publishing.tiles[0].id.clone();
    controller.send(Command::Watch {
        id: id.clone(),
        enabled: true,
    });
    wait(
        controller,
        "Controller did not decode the selected stream.",
        |s| Ok(s.sharing && watch_frames(s, &id).is_some_and(|frames| frames > 5)),
    )?;
    println!(r#"{{"phase":"publishing-and-watching"}}"#);

    // The harness now removes only this RTC participant. No more Start or Watch commands.
    let recovered = wait(
        controller,
        "Controller did not restore the media connection.",
        |s| {
            Ok(s.connection == "connected"
                && s.protected
                && descriptor(s)?.is_some_and(|d| d != original))
        },
    )?;
    ensure!(
        !recovered.sharing && !recovered.capture_pending,
        "Capture resumed after interruption."
    );
    let restored = descriptor(&recovered)?.context("Missing restored session descriptor.")?;
    let mut baseline = None;
    wait(
        controller,
        "Controller did not resume selected playback.",
        |s| {
            ensure!(
                !s.sharing && !s.capture_pending,
                "Capture resumed without a Start command."
            );
            Ok(watch_frames(s, &id)
                .is_some_and(|frames| frames.saturating_sub(*baseline.get_or_insert(frames)) > 5))
        },
    )?;
    println!(r#"{{"phase":"resumed-watch"}}"#);

    // An authoritative room-session revocation must stop automatic admission.
    wait(
        controller,
        "Controller did not stop after session revocation.",
        |s| {
            ensure!(
                !s.sharing && !s.capture_pending,
                "Capture resumed after revocation."
            );
            ensure!(
                descriptor(s)?.is_none_or(|d| d == restored),
                "Controller opened another protected session."
            );
            Ok(s.connection == "disconnected"
                && s.protected_access_ended
                && !s.busy
                && s.profile_access.is_none())
        },
    )?;
    Ok(())
}

pub(crate) fn run() -> Result<()> {
    let mut input = String::new();
    std::io::stdin().take(8193).read_to_string(&mut input)?;
    ensure!(input.len() <= 8192, "Recovery fixture input is too large.");
    let request: Request = serde_json::from_str(&input)
        .map_err(|_| anyhow::anyhow!("Invalid recovery fixture input."))?;
    let (legacy, private) = protection::unwrap_invitation(&request.invitation_url)?;
    let (origin, _) = admission::invitation_context(&legacy)?;
    let url = url::Url::parse(&origin)?;
    ensure!(
        private.is_some()
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "localhost")),
        "Recovery qualification requires a private loopback room."
    );
    SYNTHETIC_ONLY.store(true, Ordering::Release);
    let controller = Controller::new();
    let result = exercise(&controller, request.invitation_url);
    controller.begin_shutdown();
    let deadline = Instant::now() + Duration::from_secs(12);
    while !controller.shutdown_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    result?;
    ensure!(
        controller.shutdown_finished(),
        "Controller cleanup timed out."
    );
    println!(r#"{{"phase":"revoked-and-closed"}}"#);
    Ok(())
}
