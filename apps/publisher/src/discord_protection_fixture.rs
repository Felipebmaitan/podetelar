//! Debug/qualification-only entry point for an isolated backend with synthetic voice presence.
use crate::{
    account::Credential,
    admission::Admission,
    following::{Location, Target},
    protection,
};
use anyhow::{Result, ensure};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Voice {
    credential: Credential,
    location: Location,
}

impl Voice {
    pub(super) async fn join(self, invitation: &str, name: &str) -> Result<Admission> {
        let server = self.credential.server.clone();
        let url =
            url::Url::parse(&server).map_err(|_| anyhow::anyhow!("Invalid fixture server."))?;
        ensure!(
            crate::admission::valid_server(&server)
                && url.scheme() == "http"
                && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
            "Discord qualification requires a local HTTP backend."
        );
        Admission::join_target(
            invitation,
            name,
            Some(self.credential),
            Some(Target {
                server,
                location: self.location,
            }),
        )
        .await
    }
}

pub(super) async fn create(voice: Voice) -> Result<()> {
    let admitted = voice.join("", "Voice organizer").await?;
    let result = admitted
        .room_invitation_url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("This voice room has no guest invitation."))
        .and_then(protection::protect_invitation);
    admitted.leave().await;
    println!("{}", serde_json::json!({ "invitationUrl": result? }));
    Ok(())
}
