use anyhow::{Context, Result, bail, ensure};
use reqwest::{Client, Response, redirect::Policy};
use serde::{Deserialize, de::DeserializeOwned};
use std::time::Duration;
use url::Url;

// Credentials stay in this client's in-memory cookie jar. Never derive Debug
// or pass invitations, cookies or media tokens to a process command line.
pub struct Admission {
    client: Client,
    origin: Url,
    room_id: String,
    pub room_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAccess {
    pub server_url: String,
    pub token: String,
}

#[derive(Deserialize)]
struct Session {
    room: Room,
}

#[derive(Deserialize)]
struct Room {
    name: String,
}

struct Invitation {
    origin: Url,
    room_id: String,
    secret: String,
}

impl Invitation {
    fn parse(input: &str) -> Result<Self> {
        ensure!(input.len() <= 2048, "Invitation link is too long.");
        let mut url = Url::parse(input.trim()).context("Enter a valid invitation link.")?;
        let local = matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        );
        ensure!(
            url.scheme() == "https" || (local && url.scheme() == "http"),
            "Use HTTPS, or HTTP on localhost for development."
        );
        ensure!(
            url.username().is_empty() && url.password().is_none() && url.query().is_none(),
            "Invitation URL must not contain credentials or a query."
        );
        let room_id = url
            .path()
            .strip_prefix("/r/")
            .unwrap_or_default()
            .to_owned();
        let secret = url
            .fragment()
            .and_then(|fragment| fragment.strip_prefix("invite="))
            .unwrap_or_default()
            .to_owned();
        ensure!(
            valid_token(&room_id, 22) && valid_token(&secret, 43),
            "The link must contain a room invitation."
        );
        url.set_path("/");
        url.set_fragment(None);
        Ok(Self {
            origin: url,
            room_id,
            secret,
        })
    }
}

fn valid_token(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn media_diagnostic(error: impl std::fmt::Display) -> String {
    // CLI diagnostics are useful for native integration tests, but SDK errors
    // can carry signaling URLs or SDP. Remove authority before emitting text.
    let jwt = regex::Regex::new(r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+")
        .expect("JWT diagnostic pattern");
    let authority = regex::Regex::new(
        r"(?i)(access_token|token|secret|password|api_key|join_request)=[^&\s]+|ice-(pwd|ufrag):[^\s]+",
    )
    .expect("authority diagnostic pattern");
    let message = error.to_string();
    let message = jwt.replace_all(&message, "[redacted JWT]");
    let message = authority.replace_all(&message, "[redacted authority]");
    message.chars().take(300).collect()
}

impl Admission {
    pub async fn join(invitation: &str, display_name: &str) -> Result<Self> {
        let invitation = Invitation::parse(invitation)?;
        let client = Client::builder()
            .cookie_store(true)
            .redirect(Policy::none())
            .timeout(Duration::from_secs(12))
            .build()
            .context("Could not initialize secure room access.")?;
        let response = client
            .post(
                invitation
                    .origin
                    .join(&format!("api/rooms/{}/join", invitation.room_id))?,
            )
            .header("Origin", invitation.origin.origin().ascii_serialization())
            .json(
                &serde_json::json!({"invitation": invitation.secret, "displayName": display_name}),
            )
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Could not reach the room server."))?;
        let session: Session = read_response(response).await?;
        Ok(Self {
            client,
            origin: invitation.origin,
            room_id: invitation.room_id,
            room_name: session.room.name,
        })
    }

    pub async fn media_access(&self) -> Result<MediaAccess> {
        let access: MediaAccess = read_response(self.post("media-token").await?).await?;
        let media_url = Url::parse(&access.server_url)
            .context("Room server returned an invalid media address.")?;
        ensure!(
            media_url.username().is_empty() && media_url.password().is_none(),
            "Invalid media address credentials."
        );
        ensure!(
            media_url.scheme() == "wss"
                || (self.origin.scheme() == "http"
                    && media_url.scheme() == "ws"
                    && matches!(
                        media_url.host_str(),
                        Some("localhost" | "127.0.0.1" | "::1" | "[::1]")
                    )),
            "Media connections require TLS outside localhost."
        );
        Ok(access)
    }

    async fn post(&self, action: &str) -> Result<Response> {
        self.client
            .post(
                self.origin
                    .join(&format!("api/rooms/{}/{action}", self.room_id))?,
            )
            .header("Origin", self.origin.origin().ascii_serialization())
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Could not reach the room server."))
    }

    pub async fn leave(&self) {
        let _ = self.post("leave").await;
    }
}

async fn read_response<T: DeserializeOwned>(mut response: Response) -> Result<T> {
    if !response.status().is_success() {
        bail!(match response.status().as_u16() {
            401 | 404 => "Room access was denied. Check your invitation or join again.",
            429 => "Too many room requests. Wait a minute and try again.",
            _ => "The room server could not complete that request.",
        });
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .context("Room response was interrupted.")?
    {
        ensure!(
            body.len() + chunk.len() <= 16_384,
            "Room response was too large."
        );
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).context("Room server returned an invalid response.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(origin: &str) -> String {
        format!("{origin}/r/{}#invite={}", "r".repeat(22), "s".repeat(43))
    }

    #[test]
    fn invitation_authority_never_enters_request_origin_or_path() {
        let invitation = Invitation::parse(&link("https://example.test:8443")).unwrap();
        assert_eq!(invitation.origin.as_str(), "https://example.test:8443/");
        assert_eq!(invitation.room_id, "r".repeat(22));
        assert_eq!(invitation.secret, "s".repeat(43));
    }

    #[test]
    fn rejects_remote_cleartext_credentials_and_malformed_invitations() {
        for origin in [
            "http://example.test",
            "https://user:password@example.test",
            "file://example.test",
        ] {
            assert!(Invitation::parse(&link(origin)).is_err());
        }
        assert!(Invitation::parse(&link("http://localhost:3000")).is_ok());
        assert!(Invitation::parse("https://example.test/r/guess#invite=guess").is_err());
    }

    #[test]
    fn diagnostics_remove_query_tokens_encoded_join_requests_and_sdp_authority() {
        let message = media_diagnostic(
            "ws://localhost/rtc?access_token=PRIVATE&join_request=ENCODED a=ice-pwd:PRIVATE a=ice-ufrag:PRIVATE eyJheader.payload.signature",
        );
        assert!(!message.contains("PRIVATE"));
        assert!(!message.contains("ENCODED"));
        assert!(!message.contains("eyJheader"));
    }
}
