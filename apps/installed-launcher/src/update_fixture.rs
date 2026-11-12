//! Local qualification publisher substitute. Never included in installer staging.
#![cfg_attr(windows, windows_subsystem = "windows")]
#[path = "../../update_protocol.rs"]
mod protocol;
use protocol::{Action, Phase, Status};
use std::{
    io::Write,
    net::TcpStream,
    time::{Duration, Instant},
};

fn request(action: Action, token: &str) -> std::io::Result<Status> {
    let mut stream = TcpStream::connect(std::env::var(protocol::ENDPOINT_ENV).unwrap())?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    protocol::write(
        &mut stream,
        &protocol::Request {
            token: token.into(),
            action,
        },
    )?;
    protocol::read(&mut stream)
}
fn event(value: &str) {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::var("UPDATE_FIXTURE_EVENTS").unwrap())
        .unwrap();
    writeln!(file, "{value}").unwrap();
}
fn wait(phase: Phase, token: &str) -> Status {
    let start = Instant::now();
    loop {
        let status = request(Action::Poll, token).unwrap();
        assert_ne!(status.phase, Phase::Error, "{}", status.message);
        if status.phase == phase {
            assert_eq!(protocol::notes(&status.notes), status.notes);
            return status;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "Timed out waiting for updater"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn main() {
    let manifest = std::fs::read_to_string(
        std::env::current_exe()
            .unwrap()
            .with_file_name("sq.version"),
    )
    .unwrap();
    if manifest.contains("0.1.0-preview.2") {
        event("updated");
        return;
    }
    let token = std::env::var(protocol::TOKEN_ENV).unwrap();
    wait(Phase::Available, &token);
    event("available");
    assert!(request(Action::Download, &"0".repeat(64)).is_err());
    request(Action::Restart, &token).unwrap();
    assert_eq!(
        request(Action::Poll, &token).unwrap().phase,
        Phase::Available
    );
    request(Action::Download, &token).unwrap();
    wait(Phase::Ready, &token);
    event("ready");
    if std::env::var("UPDATE_FIXTURE_MODE").as_deref() == Ok("later") {
        event("later");
        return;
    }
    request(Action::Restart, &token).unwrap();
    wait(Phase::Restarting, &token);
    event("restart");
}
