//! Browser links carry only a one-use handoff ticket, never a permanent invitation.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use url::Url;

/// A window owns its request through cancellation and full worker termination.
pub struct Load {
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
    worker: Option<std::thread::JoinHandle<Option<Result<String, String>>>>,
}

impl Load {
    pub fn start(link: Link) -> Self {
        Self::with_request(async move { link.redeem().await })
    }

    pub(crate) fn with_request(
        request: impl std::future::Future<Output = Result<String>> + Send + 'static,
    ) -> Self {
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        let worker = std::thread::Builder::new()
            .name("freedomstream-invitation".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(_) => return Some(Err("Could not prepare app invitation.".into())),
                };
                runtime.block_on(async {
                    tokio::select! {
                        biased;
                        _ = cancelled => None,
                        result = request => Some(result.map_err(|error| error.to_string())),
                    }
                })
            })
            .expect("invitation worker thread");
        Self {
            cancel: Some(cancel),
            worker: Some(worker),
        }
    }

    pub fn cancel(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }

    pub fn shutdown_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(crate::worker::finished)
    }

    pub fn poll(&mut self) -> Option<Result<String, String>> {
        if !self.shutdown_finished() {
            return None;
        }
        self.worker
            .take()?
            .join()
            .unwrap_or_else(|_| Some(Err("Could not load app invitation.".into())))
    }
}

impl Drop for Load {
    fn drop(&mut self) {
        self.cancel();
        #[cfg(test)]
        crate::worker::drain_fixture(self.worker.take());
    }
}

pub struct Link {
    pub origin: Url,
    ticket: String,
}
impl Link {
    pub fn known_server(&self, server: &str) -> bool {
        crate::settings::valid_server(server)
            && Url::parse(server).is_ok_and(|url| url.origin() == self.origin.origin())
    }
    pub fn parse(input: &str) -> Result<Self> {
        ensure!(input.len() <= 4096, "Invalid browser invitation.");
        let url = Url::parse(input).context("Invalid browser invitation.")?;
        ensure!(
            url.scheme() == "freedomstream"
                && url.host_str() == Some("join")
                && matches!(url.path(), "" | "/")
                && url.username().is_empty()
                && url.password().is_none()
                && url.port().is_none(),
            "Invalid browser invitation."
        );
        let pairs: Vec<_> = url.query_pairs().collect();
        ensure!(
            pairs.len() == 1 && pairs[0].0 == "server",
            "Invalid browser invitation."
        );
        let origin = Url::parse(&pairs[0].1).context("Invalid app invitation server.")?;
        let local = matches!(
            origin.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        );
        ensure!(
            (origin.scheme() == "https" || (local && origin.scheme() == "http"))
                && origin.host_str().is_some()
                && origin.username().is_empty()
                && origin.password().is_none()
                && matches!(origin.path(), "" | "/")
                && origin.query().is_none()
                && origin.fragment().is_none(),
            "Use an HTTPS app invitation server."
        );
        let ticket = url
            .fragment()
            .and_then(|value| value.strip_prefix("handoff="))
            .unwrap_or_default();
        ensure!(
            ticket.len() == 43
                && ticket
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')),
            "Invalid app invitation ticket."
        );
        Ok(Self {
            origin,
            ticket: ticket.to_owned(),
        })
    }
    pub async fn redeem(&self) -> Result<String> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .context("Could not prepare app invitation.")?;
        let mut response = client
            .post(self.origin.join("api/app-links/redeem")?)
            .header("Origin", self.origin.origin().ascii_serialization())
            .json(&serde_json::json!({ "ticket": self.ticket }))
            .send()
            .await
            .map_err(|_| {
                anyhow::anyhow!("Could not load app invitation. Open it from your browser again.")
            })?;
        ensure!(
            response.status().is_success(),
            "App invitation expired or was refused. Open it from your browser again."
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| anyhow::anyhow!("Could not load app invitation."))?
        {
            ensure!(
                bytes.len() + chunk.len() <= 4096,
                "Invalid app invitation response."
            );
            bytes.extend_from_slice(&chunk);
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Reply {
            invitation_url: String,
        }
        let reply: Reply =
            serde_json::from_slice(&bytes).context("Invalid app invitation response.")?;
        ensure!(
            crate::admission::valid_invitation(&reply.invitation_url)
                && Url::parse(&reply.invitation_url)?.origin() == self.origin.origin(),
            "Invalid app invitation response."
        );
        Ok(reply.invitation_url)
    }
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let executable = std::env::var_os("FREEDOMSTREAM_PORTABLE_EXE")
        .context("Download and run the portable EXE to enable browser links.")?;
    let mut command = std::process::Command::new(executable);
    command.arg(if enabled {
        "--enable-browser-links"
    } else {
        "--disable-browser-links"
    });
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command
        .output()
        .context("Could not update browser-link settings.")?;
    ensure!(
        output.status.success(),
        "Could not update browser links. Check that this app has permission to save your user settings."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelling_a_pending_load_drops_the_request_before_worker_completion() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        use std::time::{Duration, Instant};
        struct Request(Arc<AtomicBool>);
        impl Drop for Request {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let dropped = Arc::new(AtomicBool::new(false));
        let request = Request(dropped.clone());
        let (started, ready) = mpsc::channel();
        let mut load = Load::with_request(async move {
            let _request = request;
            started.send(()).unwrap();
            std::future::pending::<Result<String>>().await
        });
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        load.cancel();
        let started = Instant::now();
        while !load.shutdown_finished() {
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(dropped.load(Ordering::Acquire));
        assert!(load.poll().is_none());
    }

    #[test]
    fn completed_load_can_be_collected_only_once() {
        let mut load = Load::with_request(async { Ok("synthetic result, no invitation".into()) });
        let start = std::time::Instant::now();
        let received = loop {
            if let Some(result) = load.poll() {
                break result.unwrap();
            }
            assert!(start.elapsed() < std::time::Duration::from_secs(3));
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        assert_eq!(received, "synthetic result, no invitation");
        assert!(load.poll().is_none());
    }

    #[test]
    #[ignore = "requires a live FreedomStream API and local fixture path"]
    fn redeem_real_server_invitation() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            link: String,
            invitation: String,
        }
        let path = std::env::var_os("FREEDOMSTREAM_APP_LINK_FIXTURE").expect("Local fixture path");
        let fixture: Fixture =
            serde_json::from_slice(&std::fs::read(path).expect("Read local fixture"))
                .expect("Valid local fixture");
        let collect = |load: &mut Load| {
            let started = std::time::Instant::now();
            loop {
                if let Some(result) = load.poll() {
                    break result;
                }
                assert!(
                    started.elapsed() < std::time::Duration::from_secs(15),
                    "Native handoff worker timed out"
                );
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        let mut load = Load::start(Link::parse(&fixture.link).expect("Valid local browser link"));
        let received = collect(&mut load).expect("Redeem live app invitation");
        assert!(
            received == fixture.invitation,
            "Native invitation must match the live server"
        );
        assert!(
            collect(&mut Load::start(Link::parse(&fixture.link).unwrap())).is_err(),
            "Replaying a native handoff must fail"
        );
    }

    #[test]
    fn accepts_windows_shell_canonical_root_but_rejects_other_paths() {
        let ticket = "t".repeat(43);
        for path in ["", "/"] {
            let link = Link::parse(&format!(
                "freedomstream://join{path}?server=http%3A%2F%2Flocalhost%3A18321#handoff={ticket}"
            ))
            .expect("Windows may append a slash to a protocol authority");
            assert_eq!(link.origin.as_str(), "http://localhost:18321/");
            assert_eq!(link.ticket, ticket);
        }
        for path in ["/room", "//", "/%2F", "/%6aoin"] {
            assert!(
                Link::parse(&format!(
                    "freedomstream://join{path}?server=https%3A%2F%2Fexample.test#handoff={ticket}"
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn accepts_only_explicit_secure_origins_and_bounded_tickets() {
        let ticket = "t".repeat(43);
        assert!(
            Link::parse(&format!(
                "freedomstream://join?server=https%3A%2F%2Fexample.test#handoff={ticket}"
            ))
            .is_ok()
        );
        assert!(
            Link::parse(&format!(
                "freedomstream://join?server=http%3A%2F%2Flocalhost%3A3211#handoff={ticket}"
            ))
            .is_ok()
        );
        for server in [
            "http://example.test",
            "https://user@example.test",
            "https://example.test/path",
            "file:///C:/",
            "https://example.test/?query",
            "https://example.test/#fragment",
        ] {
            let encoded: String = url::form_urlencoded::byte_serialize(server.as_bytes()).collect();
            assert!(
                Link::parse(&format!(
                    "freedomstream://join?server={encoded}#handoff={ticket}"
                ))
                .is_err()
            );
        }
        for link in [
            "freedomstream://join",
            "freedomstream://other?server=https://example.test",
            "freedomstream://join?server=https://example.test&extra=1#handoff=bad",
            "https://example.test",
        ] {
            assert!(Link::parse(link).is_err());
        }
    }

    #[test]
    fn saved_server_trust_requires_the_same_canonical_origin() {
        let link = Link::parse(&format!(
            "freedomstream://join?server=https%3A%2F%2Fexample.test#handoff={}",
            "t".repeat(43)
        ))
        .unwrap();
        let invitation = format!(
            "https://example.test/r/{}#invite={}",
            "r".repeat(22),
            "s".repeat(43)
        );
        assert!(!link.known_server(&invitation));
        assert!(link.known_server("https://example.test"));
        assert!(!link.known_server(&invitation.replace("example.test", "another.test")));
        assert!(!link.known_server(&invitation.replace("example.test", "example.test:8443")));
        assert!(!link.known_server("https://another.test"));
        assert!(!link.known_server("https://example.test:8443"));
        assert!(!link.known_server("https://user@example.test"));
        assert!(!link.known_server("https://example.test?secret=bad"));
        assert!(!link.known_server("https://example.test#secret"));
    }
}
