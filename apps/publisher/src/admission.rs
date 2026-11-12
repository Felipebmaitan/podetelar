use anyhow::{Context, Result, bail, ensure};
use reqwest::{Client, Response, redirect::Policy};
use serde::{Deserialize, de::DeserializeOwned};
use std::time::Duration;
use url::Url;

pub(crate) fn valid_server(input: &str) -> bool {
    let Ok(url) = Url::parse(input) else {
        return false;
    };
    let local = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    (url.scheme() == "https" || (local && url.scheme() == "http"))
        && url.origin().ascii_serialization() == input
        && url.username().is_empty()
        && url.password().is_none()
}

// Credentials stay in this client's in-memory cookie jar. Never derive Debug
// or pass invitations, cookies or media tokens to a process command line.
#[derive(Clone)]
pub struct Admission {
    client: Client,
    origin: Url,
    room_id: String,
    pub room_name: String,
}
struct ProbeLease {
    client: Client,
    url: Url,
    origin: String,
    token: String,
}
impl Drop for ProbeLease {
    fn drop(&mut self) {
        let (client, url, origin, token) = (
            self.client.clone(),
            self.url.clone(),
            self.origin.clone(),
            self.token.clone(),
        );
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = client
                    .delete(url)
                    .header("Origin", origin)
                    .header("X-Network-Probe", token)
                    .timeout(Duration::from_secs(2))
                    .send()
                    .await;
            });
        }
    }
}

pub enum PreviewDelivery {
    Sent,
    Retry(Duration),
    Denied,
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

pub fn valid_invitation(input: &str) -> bool {
    Invitation::parse(input).is_ok()
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
    pub async fn stream_diagnostic(
        &self,
        method: reqwest::Method,
        owner: &str,
        sid: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value> {
        ensure!(
            owner.len() <= 128
                && sid.len() <= 131
                && owner
                    .bytes()
                    .chain(sid.bytes())
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'),
            "Invalid publication."
        );
        let url = self.origin.join(&format!(
            "api/rooms/{}/stream-diagnostics/{owner}/{sid}",
            self.room_id
        ))?;
        let mut request = self
            .client
            .request(method, url)
            .header("Origin", self.origin.origin().ascii_serialization())
            .timeout(Duration::from_secs(2));
        if let Some(body) = body {
            request = request.json(body);
        }
        // A bounded retry avoids phase-locking several three-second samplers
        // against the server's per-publication exclusion. Never delay media work.
        let response = tokio::time::timeout(Duration::from_secs(2), async {
            let mut response = request
                .try_clone()
                .context("Diagnostic request unavailable.")?
                .send()
                .await?;
            for _ in 0..2 {
                if response.status() != reqwest::StatusCode::TOO_MANY_REQUESTS {
                    break;
                }
                let jitter = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos() as u64
                    % 250;
                tokio::time::sleep(Duration::from_millis(100 + jitter)).await;
                response = request
                    .try_clone()
                    .context("Diagnostic request unavailable.")?
                    .send()
                    .await?;
            }
            Ok::<_, anyhow::Error>(response)
        })
        .await??;
        ensure!(response.status().is_success(), "Diagnostics unavailable.");
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(serde_json::Value::Null);
        }
        ensure!(
            response.content_length().unwrap_or(0) <= 8192,
            "Diagnostic response too large."
        );
        use futures_util::StreamExt;
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            ensure!(
                bytes.len() + chunk.len() <= 8192,
                "Diagnostic response too large."
            );
            bytes.extend_from_slice(&chunk);
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn measure_connection(&self) -> crate::network::Capacity {
        use crate::network::{Capacity, PROBE_BYTES, PROBE_CAP_BPS};
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Ticket {
            probe: String,
            payload_cap_bps: u64,
            byte_limit: usize,
        }
        #[derive(Deserialize)]
        struct Upload {
            bps: u64,
            bytes: usize,
            seconds: f64,
        }
        let mut measured = Capacity::default();
        let mut unavailable = "Connection measurement unavailable. Try again in Advanced.";
        let result: Result<()> = async {
            let started = std::time::Instant::now();
            let ticket: Ticket = loop {
                let response = self
                    .client
                    .post(
                        self.origin
                            .join(&format!("api/rooms/{}/network-test", self.room_id))?,
                    )
                    .header("Origin", self.origin.origin().ascii_serialization())
                    .header("X-Network-Test-Max-Bps", PROBE_CAP_BPS.to_string())
                    .send()
                    .await?;
                if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    unavailable = "Wait 30 seconds before testing again.";
                }
                if response.status() == reqwest::StatusCode::CONFLICT
                    && started.elapsed() < Duration::from_secs(25)
                {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
                break read_response(response).await?;
            };
            ensure!(
                ticket.probe.len() == 48 && ticket.probe.bytes().all(|b| b.is_ascii_hexdigit()),
                "Invalid connection test."
            );
            ensure!(
                (ticket.payload_cap_bps == 100_000_000 && ticket.byte_limit == 24 * 1024 * 1024)
                    || (ticket.payload_cap_bps == PROBE_CAP_BPS
                        && ticket.byte_limit == PROBE_BYTES),
                "Invalid connection test limits."
            );
            let cap = ticket.payload_cap_bps;
            let byte_limit = ticket.byte_limit;
            let path = format!("api/rooms/{}/network-test", self.room_id);
            let _lease = ProbeLease {
                client: self.client.clone(),
                url: self.origin.join(&path)?,
                origin: self.origin.origin().ascii_serialization(),
                token: ticket.probe.clone(),
            };
            let mut response = self
                .client
                .get(self.origin.join(&(path.clone() + "/download"))?)
                .header("X-Network-Probe", &ticket.probe)
                .timeout(Duration::from_secs(7))
                .send()
                .await?;
            ensure!(
                response.status().is_success(),
                "Connection test unavailable."
            );
            let began = std::time::Instant::now();
            let mut bytes = 0usize;
            while let Some(chunk) = response.chunk().await? {
                bytes += chunk.len();
                ensure!(bytes <= byte_limit, "Connection test exceeded limit.");
            }
            ensure!(bytes >= 65_536, "Connection measurement too small.");
            measured.download_bps = Some(
                ((bytes as f64 * 8.0 / began.elapsed().as_secs_f64().max(0.001)) as u64).min(cap),
            );
            let stream = futures_util::stream::unfold(
                (
                    0usize,
                    std::time::Instant::now(),
                    bytes::Bytes::from(vec![0x5a; 65_536]),
                ),
                move |(sent, start, chunk)| async move {
                    if sent >= byte_limit || start.elapsed() >= Duration::from_secs(2) {
                        return None;
                    }
                    let due = Duration::from_secs_f64(sent as f64 * 8.0 / cap as f64);
                    if let Some(wait) = due.checked_sub(start.elapsed()) {
                        tokio::time::sleep(wait).await;
                    }
                    let size = (byte_limit - sent).min(65_536);
                    Some((
                        Ok::<_, std::io::Error>(chunk.slice(..size)),
                        (sent + size, start, chunk),
                    ))
                },
            );
            let response = self
                .client
                .post(self.origin.join(&(path + "/upload"))?)
                .header("Origin", self.origin.origin().ascii_serialization())
                .header("X-Network-Probe", ticket.probe)
                .header("Content-Type", "application/octet-stream")
                .body(reqwest::Body::wrap_stream(stream))
                .timeout(Duration::from_secs(7))
                .send()
                .await?;
            let uploaded: Upload = read_response(response).await?;
            ensure!(
                uploaded.bytes >= 65_536
                    && uploaded.bytes <= byte_limit
                    && uploaded.seconds > 0.0
                    && uploaded.seconds <= 4.0,
                "Invalid upload measurement."
            );
            measured.upload_bps = Some(uploaded.bps.min(cap));
            Ok(())
        }
        .await;
        if result.is_err() {
            measured.error = unavailable.into();
        }
        measured
    }
    pub async fn join(invitation: &str, display_name: &str) -> Result<Self> {
        let mut invitation = Invitation::parse(invitation)?;
        let client = Client::builder()
            .cookie_store(true)
            .redirect(Policy::none())
            .timeout(Duration::from_secs(12))
            .build()
            .context("Could not initialize secure room access.")?;
        // Resolve before sending any invitation/session authority. Older servers
        // lack discovery; failures other than 404 must never silently use a CDN.
        let connection = client
            .get(invitation.origin.join("api/connection")?)
            .send()
            .await
            .context("Could not resolve the direct room server.")?;
        if connection.status() != reqwest::StatusCode::NOT_FOUND {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Connection {
                direct_url: String,
            }
            let connection: Connection = read_response(connection).await?;
            ensure!(
                valid_server(&connection.direct_url),
                "Invalid direct room server."
            );
            let direct = Url::parse(&connection.direct_url)?;
            ensure!(
                invitation.origin.scheme() != "https" || direct.scheme() == "https",
                "Direct room access requires HTTPS."
            );
            invitation.origin = direct;
        }
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
        let response = self.post("native-media-token").await?;
        ensure!(
            response.status().as_u16() != 404,
            "This room server needs an update before native playback can be used."
        );
        let access: MediaAccess = read_response(response).await?;
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

    pub async fn send_preview(&self, sid: &str, bytes: Vec<u8>) -> PreviewDelivery {
        let url = self
            .origin
            .join(&format!("api/rooms/{}/previews/screen", self.room_id))
            .expect("validated room URL");
        let response = self
            .client
            .post(url)
            .header("Origin", self.origin.origin().ascii_serialization())
            .header("Content-Type", "image/jpeg")
            .header("X-Track-Sid", sid)
            .timeout(Duration::from_secs(5))
            .body(bytes)
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => PreviewDelivery::Sent,
            Ok(response)
                if response.status().as_u16() == 401 || response.status().as_u16() == 403 =>
            {
                PreviewDelivery::Denied
            }
            Ok(response) => PreviewDelivery::Retry(Duration::from_secs(
                response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or(3)
                    .clamp(1, 60),
            )),
            Err(_) => PreviewDelivery::Retry(Duration::from_secs(3)),
        }
    }
    pub async fn get_preview(
        &self,
        owner: &str,
        revision: &str,
    ) -> Result<eframe::egui::ColorImage> {
        ensure!(
            owner.len() == 36
                && owner.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-')
                && valid_token(revision, 22),
            "Invalid preview address."
        );
        let url = self.origin.join(&format!(
            "api/rooms/{}/previews/{owner}/screen/{revision}",
            self.room_id
        ))?;
        let mut response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Preview is unavailable."))?;
        ensure!(response.status().is_success(), "Preview is unavailable.");
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len() + chunk.len() <= 262_144,
                "Preview exceeds its size limit."
            );
            bytes.extend_from_slice(&chunk);
        }
        let dimensions =
            image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Jpeg)
                .into_dimensions()?;
        ensure!(
            dimensions.0 <= 960 && dimensions.1 <= 540 && dimensions.0 > 0 && dimensions.1 > 0,
            "Preview dimensions are invalid."
        );
        let image =
            image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)?.into_rgba8();
        Ok(eframe::egui::ColorImage::from_rgba_unmultiplied(
            [dimensions.0 as usize, dimensions.1 as usize],
            image.as_raw(),
        ))
    }

    pub async fn remove_preview(&self, sid: &str) {
        let url = self
            .origin
            .join(&format!("api/rooms/{}/previews/screen", self.room_id))
            .expect("validated room URL");
        let _ = self
            .client
            .delete(url)
            .header("Origin", self.origin.origin().ascii_serialization())
            .header("X-Track-Sid", sid)
            .timeout(Duration::from_secs(2))
            .send()
            .await;
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

    #[tokio::test]
    #[ignore = "requires two independently running API backends"]
    async fn invitation_links_select_two_live_backends_without_saved_configuration() {
        #[derive(Deserialize)]
        struct Fixture {
            link: String,
            origin: String,
            name: String,
        }
        let path =
            std::env::var_os("FREEDOMSTREAM_MULTI_SERVER_FIXTURE").expect("Local fixture path");
        let fixtures: Vec<Fixture> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(fixtures.len(), 2);
        assert_ne!(fixtures[0].origin, fixtures[1].origin);
        let mut admissions = Vec::new();
        for fixture in &fixtures {
            let link = crate::browser_links::Link::parse(&fixture.link).unwrap();
            let invitation = link
                .redeem()
                .await
                .expect("Redeem this backend's one-use link");
            let admission = Admission::join(&invitation, "Session-only nickname")
                .await
                .expect("Admit on invitation-selected backend");
            assert_eq!(
                admission.origin.origin().ascii_serialization(),
                fixture.origin
            );
            assert_eq!(admission.room_name, fixture.name);
            assert!(link.redeem().await.is_err(), "Handoff remains single-use");
            admissions.push(admission);
        }
        let foreign = admissions[1]
            .origin
            .join(&format!("api/rooms/{}/session", admissions[1].room_id))
            .unwrap();
        assert_eq!(
            admissions[0]
                .client
                .get(foreign)
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
        for admission in admissions {
            admission.leave().await;
        }
    }

    #[tokio::test]
    #[ignore = "requires a live split-origin API fixture"]
    async fn joins_discovered_direct_server() {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Fixture {
            invitation: String,
            direct_url: String,
        }
        let path = std::env::var_os("FREEDOMSTREAM_DIRECT_FIXTURE").expect("Local fixture path");
        let fixture: Fixture =
            serde_json::from_slice(&std::fs::read(path).expect("Read local fixture"))
                .expect("Valid fixture");
        let admission = Admission::join(&fixture.invitation, "Native direct verification")
            .await
            .expect("Join direct API");
        assert_eq!(
            admission.origin.origin().ascii_serialization(),
            fixture.direct_url
        );
        let measured = admission.measure_connection().await;
        assert!(
            measured.download_bps.is_some(),
            "Direct download measurement must complete"
        );
        assert!(
            measured.upload_bps.is_some(),
            "Direct upload measurement must complete"
        );
        admission.leave().await;
    }

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
