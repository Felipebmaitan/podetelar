//! Test-only publisher substitute; not included by installer packaging.
#![cfg_attr(windows, windows_subsystem = "windows")]
#[path = "../../activation_protocol.rs"]
mod protocol;
fn event(text: &str) {
    use std::io::Write;
    writeln!(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(std::env::var("LINK_FIXTURE_EVENTS").unwrap())
            .unwrap(),
        "{text}"
    )
    .unwrap();
}
fn main() {
    event("started");
    let address = std::env::var(protocol::ENDPOINT).unwrap();
    let token = std::env::var(protocol::TOKEN).unwrap();
    assert!(std::env::var_os(protocol::INSTALLED).is_some());
    assert!(protocol::exchange(&address, &"0".repeat(64), true, None).is_err());
    let started = std::time::Instant::now();
    while started.elapsed() < std::time::Duration::from_secs(20) {
        let reply = protocol::exchange(&address, &token, true, None).unwrap();
        if let Some(link) = reply.link {
            assert!(protocol::valid(&link));
            event("received");
            return;
        }
        if reply.focus {
            event("focused");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("No activation received");
}
