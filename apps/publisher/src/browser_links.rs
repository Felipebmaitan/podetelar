//! Browser links carry only a one-use handoff ticket, never a permanent invitation.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use url::Url;

pub struct Link {
    pub origin: Url,
    ticket: String,
}
impl Link {
    pub fn parse(input: &str) -> Result<Self> {
        ensure!(input.len() <= 4096, "Invalid browser invitation.");
        let url = Url::parse(input).context("Invalid browser invitation.")?;
        ensure!(
            url.scheme() == "freedomstream"
                && url.host_str() == Some("join")
                && url.path().is_empty()
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
        let link = Link::parse(&fixture.link).expect("Valid local browser link");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let received = runtime
            .block_on(link.redeem())
            .expect("Redeem live app invitation");
        assert!(
            received == fixture.invitation,
            "Native invitation must match the live server"
        );
        assert!(
            runtime.block_on(link.redeem()).is_err(),
            "Replaying a native handoff must fail"
        );
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
}
