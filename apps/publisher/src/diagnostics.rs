use std::sync::LazyLock;

struct ConnectionLog;
static LOGGER: ConnectionLog = ConnectionLog;

pub fn enable() {
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Debug);
    }
}

impl log::Log for ConnectionLog {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Debug
            && (metadata.target().starts_with("livekit::rtc_engine")
                || metadata.target() == "libwebrtc")
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata())
            && let Some(message) = connection_message(&record.args().to_string())
        {
            eprintln!("PodeTelar diagnostic: {message}");
        }
    }
    fn flush(&self) {}
}

// Allowlist structured connection facts. Never pass SDK messages/SDP/signaling
// URLs through a logger, even when diagnostic mode is explicitly enabled.
fn connection_message(message: &str) -> Option<String> {
    if message.contains("TLS post connection check failed") {
        return Some("TLS certificate verification failed".to_owned());
    }
    if let Some(state) = message.strip_prefix("connection change, ")
        && state.chars().all(|c| c.is_ascii_alphabetic() || c == ' ')
    {
        return Some(format!("state {state}"));
    }
    if message.starts_with("local ice_candidate") || message.starts_with("remote ice_candidate") {
        static CANDIDATE: LazyLock<regex::Regex> = LazyLock::new(|| {
            regex::Regex::new(
                r"candidate:[^\s]+ [0-9]+ (udp|tcp) [0-9]+ ([0-9a-fA-F.:]+) ([0-9]+) typ ([a-z]+)",
            )
            .expect("ICE diagnostic pattern")
        });
        if let Some(fields) = CANDIDATE.captures(message) {
            return Some(format!(
                "{} ICE {} {}:{} {}",
                if message.starts_with("local") {
                    "local"
                } else {
                    "remote"
                },
                &fields[1],
                &fields[2],
                &fields[3],
                &fields[4]
            ));
        }
    }
    for (prefix, label) in [
        ("received publisher answer:", "publisher answer received"),
        ("received subscriber offer:", "subscriber offer received"),
        ("received JoinResponse:", "room admitted"),
    ] {
        if message.starts_with(prefix) {
            return Some(label.to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emits_candidate_addresses_without_ice_or_signaling_authority() {
        assert_eq!(
            connection_message("TLS post connection check failed PRIVATE").as_deref(),
            Some("TLS certificate verification failed")
        );
        let message = "local ice_candidate candidate:123 1 udp 999 192.0.2.1 12345 typ host generation 0 ufrag PRIVATE Publisher";
        assert_eq!(
            connection_message(message).as_deref(),
            Some("local ICE udp 192.0.2.1:12345 host")
        );
        assert!(connection_message("sending publisher offer: a=ice-pwd:PRIVATE").is_none());
        assert_eq!(
            connection_message("received JoinResponse: token PRIVATE").as_deref(),
            Some("room admitted")
        );
    }
}
