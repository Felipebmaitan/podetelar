//! Voice discovery never starts capture or playback; the room worker owns teardown.
use crate::account::{Credential, USER_AGENT};
use reqwest::{StatusCode, header::RETRY_AFTER};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::sync::watch;

const ACTIVE_INTERVAL: Duration = Duration::from_secs(2);
const IDLE_INTERVAL: Duration = Duration::from_secs(10);
const MAX_BACKOFF: Duration = Duration::from_secs(120);
const RATE_LIMIT_DELAY: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub guild_id: String,
    pub guild_name: String,
    pub channel_id: String,
    pub channel_name: String,
    pub enabled: bool,
}
impl Location {
    pub fn key(&self) -> String {
        format!("{}:{}", self.guild_id, self.channel_id)
    }
}
#[derive(Clone)]
pub struct Target {
    pub server: String,
    pub location: Location,
}
#[derive(Clone, Default, Deserialize)]
pub struct State {
    pub known: bool,
    pub enabled: bool,
    pub locations: Vec<Location>,
}
type Desired = (u64, Option<Credential>);

#[derive(Debug, Default)]
struct Unavailable {
    retry_after: Duration,
}

impl Unavailable {
    fn from_response(response: &reqwest::Response) -> Self {
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| {
                value
                    .parse::<u64>()
                    .ok()
                    .map(Duration::from_secs)
                    .or_else(|| {
                        httpdate::parse_http_date(value)
                            .ok()
                            .map(|date| date.duration_since(SystemTime::now()).unwrap_or_default())
                    })
            });
        let retry_after = retry_after.unwrap_or_else(|| {
            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                RATE_LIMIT_DELAY
            } else {
                Duration::ZERO
            }
        });
        Self { retry_after }
    }
}

async fn poll(http: &reqwest::Client, credential: &Credential) -> Result<State, Unavailable> {
    let mut response = credential
        .authorize(http.get(format!("{}/api/auth/voice", credential.server)))
        .send()
        .await
        .map_err(|_| Unavailable::default())?;
    if !response.status().is_success() {
        return Err(Unavailable::from_response(&response));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Unavailable::default())? {
        if bytes.len() + chunk.len() > 65536 {
            return Err(Unavailable::default());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| Unavailable::default())
}

fn next_delay(result: &Result<State, Unavailable>, backoff: &mut Duration) -> Duration {
    match result {
        Ok(state) => {
            *backoff = IDLE_INTERVAL;
            if state.known
                && state.enabled
                && state.locations.iter().any(|location| location.enabled)
            {
                ACTIVE_INTERVAL
            } else {
                IDLE_INTERVAL
            }
        }
        Err(error) => {
            let delay = (*backoff).max(error.retry_after);
            *backoff = delay.saturating_mul(2).min(MAX_BACKOFF);
            delay
        }
    }
}

pub struct Client {
    desired: watch::Sender<Desired>,
    state: Arc<Mutex<State>>,
}
impl Client {
    pub fn new() -> Self {
        let (desired, mut input) = watch::channel::<Desired>((0, None));
        let state = Arc::new(Mutex::new(State::default()));
        let output = state.clone();
        std::thread::Builder::new()
            .name("podetelar-voice".into())
            .spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                runtime.block_on(async move {
                    let Ok(http) = reqwest::Client::builder()
                        .user_agent(USER_AGENT)
                        .redirect(reqwest::redirect::Policy::none())
                        .timeout(Duration::from_secs(5))
                        .build()
                    else {
                        return;
                    };
                    'credential: loop {
                        let (generation, credential) = input.borrow_and_update().clone();
                        let Some(credential) = credential else {
                            if input.changed().await.is_err() {
                                break;
                            }
                            continue;
                        };
                        let mut backoff = IDLE_INTERVAL;
                        loop {
                            let result = tokio::select! {
                                biased;
                                changed = input.changed() => {
                                    if changed.is_err() {
                                        break 'credential;
                                    }
                                    continue 'credential;
                                }
                                result = poll(&http, &credential) => result,
                            };
                            let delay = next_delay(&result, &mut backoff);
                            {
                                let current = input.borrow();
                                if current.0 == generation
                                    && let Ok(mut state) = output.lock()
                                {
                                    *state = result.unwrap_or_default();
                                }
                            }
                            tokio::select! {
                                biased;
                                changed = input.changed() => {
                                    if changed.is_err() {
                                        break 'credential;
                                    }
                                    continue 'credential;
                                }
                                () = tokio::time::sleep(delay) => {}
                            }
                        }
                    }
                });
            })
            .expect("Discord voice worker");
        Self { desired, state }
    }
    #[cfg(test)]
    pub fn disabled() -> Self {
        Self::fixture(State::default())
    }
    #[cfg(any(test, feature = "ui-review"))]
    pub fn fixture(state: State) -> Self {
        Self {
            desired: watch::channel((0, None)).0,
            state: Arc::new(Mutex::new(state)),
        }
    }
    pub fn set_credential(&self, credential: Option<Credential>) {
        self.desired.send_modify(|desired| {
            desired.0 = desired.0.wrapping_add(1);
            desired.1 = credential;
            if let Ok(mut state) = self.state.lock() {
                *state = State::default();
            }
        });
    }
    pub fn state(&self) -> State {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }
}
#[derive(Default)]
pub struct Follow {
    pub current: Option<Location>,
    pub independent: bool,
    pub paused: bool,
    observed: Option<Vec<String>>,
    pub selected: Option<String>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Leave,
    Join(Location),
}
impl Follow {
    pub fn manual_leave(&mut self, state: &State) {
        self.observe(state);
        self.current = None;
        self.independent = false;
        self.paused = true;
    }
    pub fn invitation(&mut self) {
        self.current = None;
        self.independent = true;
    }
    pub fn resume(&mut self) {
        self.paused = false;
        self.independent = false;
    }
    fn observe(&mut self, state: &State) {
        if !state.known {
            return;
        }
        let mut observed: Vec<_> = state.locations.iter().map(Location::key).collect();
        observed.sort();
        if self
            .observed
            .as_ref()
            .is_some_and(|previous| previous != &observed)
        {
            self.paused = false;
        }
        self.observed = Some(observed);
    }
    pub fn tick(
        &mut self,
        signed_in: bool,
        state: &State,
        connected: bool,
        busy: bool,
        modal: bool,
    ) -> Option<Action> {
        if !signed_in {
            self.selected = None;
            self.observed = None;
            return self.current.take().map(|_| Action::Leave);
        }
        self.observe(state);
        if self.independent || !state.known {
            return None;
        }
        let eligible: Vec<_> = state
            .locations
            .iter()
            .filter(|c| state.enabled && c.enabled)
            .collect();
        let candidate = if eligible.len() == 1 {
            Some(eligible[0])
        } else {
            eligible
                .iter()
                .copied()
                .find(|c| self.selected.as_ref() == Some(&c.key()))
                .or_else(|| {
                    eligible.iter().copied().find(|c| {
                        self.current
                            .as_ref()
                            .is_some_and(|old| old.key() == c.key())
                    })
                })
        };
        if self.paused || modal || candidate.is_none() {
            if self
                .current
                .as_ref()
                .is_some_and(|old| !eligible.iter().any(|c| c.key() == old.key()))
            {
                self.current = None;
                return Some(Action::Leave);
            }
            return None;
        }
        let candidate = candidate.unwrap();
        if self
            .current
            .as_ref()
            .is_none_or(|old| old.key() != candidate.key())
            || (!connected && !busy)
        {
            self.current = Some(candidate.clone());
            return Some(Action::Join(candidate.clone()));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn location(id: &str) -> Location {
        Location {
            guild_id: "guild".into(),
            guild_name: "Friends".into(),
            channel_id: id.into(),
            channel_name: id.into(),
            enabled: true,
        }
    }
    fn state(id: &str) -> State {
        State {
            known: true,
            enabled: true,
            locations: vec![location(id)],
        }
    }
    #[test]
    fn polls_eligible_voice_faster_than_idle_or_unknown_state() {
        let mut backoff = MAX_BACKOFF;
        let mut snapshot = state("A");
        assert_eq!(
            next_delay(&Ok(snapshot.clone()), &mut backoff),
            ACTIVE_INTERVAL
        );
        assert_eq!(backoff, IDLE_INTERVAL);
        snapshot.locations[0].enabled = false;
        assert_eq!(next_delay(&Ok(snapshot), &mut backoff), IDLE_INTERVAL);
        assert_eq!(
            next_delay(&Ok(State::default()), &mut backoff),
            IDLE_INTERVAL
        );
    }

    #[test]
    fn failures_back_off_until_a_valid_snapshot_and_respect_retry_after() {
        let mut backoff = IDLE_INTERVAL;
        let unavailable = Err(Unavailable::default());
        for seconds in [10, 20, 40, 80, 120, 120] {
            assert_eq!(
                next_delay(&unavailable, &mut backoff),
                Duration::from_secs(seconds)
            );
        }
        next_delay(&Ok(state("A")), &mut backoff);
        let retry_after = Duration::from_secs(180);
        let limited = Err(Unavailable { retry_after });
        assert_eq!(next_delay(&limited, &mut backoff), retry_after);
        next_delay(&Ok(state("A")), &mut backoff);
        assert_eq!(next_delay(&unavailable, &mut backoff), IDLE_INTERVAL);
    }

    #[tokio::test]
    async fn reads_retry_after_from_real_http_responses() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let date = httpdate::fmt_http_date(SystemTime::now() + Duration::from_secs(180));
        let server = std::thread::spawn(move || {
            for header in [
                String::new(),
                "Retry-After: 75\r\n".into(),
                format!("Retry-After: {date}\r\n"),
            ] {
                let (mut connection, _) = listener.accept().unwrap();
                connection
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = BufReader::new(&mut connection);
                let mut line = String::new();
                loop {
                    line.clear();
                    if request.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                }
                write!(connection, "HTTP/1.1 429 Too Many Requests\r\nConnection: close\r\nContent-Length: 0\r\n{header}\r\n").unwrap();
            }
        });
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        for expected in [RATE_LIMIT_DELAY, Duration::from_secs(75)] {
            let response = http.get(&url).send().await.unwrap();
            assert_eq!(Unavailable::from_response(&response).retry_after, expected);
        }
        let response = http.get(&url).send().await.unwrap();
        let delay = Unavailable::from_response(&response).retry_after;
        assert!(delay > Duration::from_secs(150) && delay <= Duration::from_secs(180));
        server.join().unwrap();
    }

    #[test]
    fn leaving_an_independent_room_uses_the_current_voice_channel_as_baseline() {
        let mut follow = Follow::default();
        follow.invitation();
        follow.manual_leave(&state("A"));
        assert_eq!(follow.tick(true, &state("A"), false, false, false), None);
        assert_eq!(
            follow.tick(true, &state("B"), false, false, false),
            Some(Action::Join(location("B")))
        );
    }

    #[test]
    fn manual_leave_during_outage_waits_for_a_confirmed_channel_change() {
        let mut follow = Follow::default();
        follow.invitation();
        follow.manual_leave(&State::default());
        assert_eq!(follow.tick(true, &state("A"), false, false, false), None);
        assert_eq!(
            follow.tick(true, &state("B"), false, false, false),
            Some(Action::Join(location("B")))
        );
    }

    #[test]
    fn moves_join_without_carrying_media_and_unknown_state_preserves_session() {
        let mut follow = Follow::default();
        assert_eq!(
            follow.tick(true, &state("A"), false, false, false),
            Some(Action::Join(location("A")))
        );
        assert_eq!(
            follow.tick(true, &State::default(), true, false, false),
            None
        );
        assert_eq!(
            follow.tick(true, &state("B"), true, false, false),
            Some(Action::Join(location("B")))
        );
        assert_eq!(
            follow.tick(
                true,
                &State {
                    known: true,
                    enabled: true,
                    ..Default::default()
                },
                true,
                false,
                false
            ),
            Some(Action::Leave)
        );
    }
    #[test]
    fn manual_leave_pauses_until_channel_changes_or_resume() {
        let mut follow = Follow::default();
        follow.tick(true, &state("A"), false, false, false);
        follow.manual_leave(&state("A"));
        assert_eq!(follow.tick(true, &state("A"), false, false, false), None);
        assert_eq!(
            follow.tick(true, &state("B"), false, false, false),
            Some(Action::Join(location("B")))
        );
        follow.manual_leave(&state("B"));
        follow.resume();
        assert_eq!(
            follow.tick(true, &state("B"), false, false, false),
            Some(Action::Join(location("B")))
        );
    }
    #[test]
    fn invitation_sessions_ignore_voice_and_logout() {
        let mut follow = Follow::default();
        follow.invitation();
        assert_eq!(follow.tick(true, &state("B"), true, false, false), None);
        assert_eq!(
            follow.tick(false, &State::default(), true, false, false),
            None
        );
    }
    #[test]
    fn multiple_channels_require_selection_and_disabled_channels_disconnect() {
        let mut follow = Follow::default();
        let mut s = state("A");
        s.locations.push(location("B"));
        assert_eq!(follow.tick(true, &s, false, false, false), None);
        follow.selected = Some(location("B").key());
        assert_eq!(
            follow.tick(true, &s, false, false, false),
            Some(Action::Join(location("B")))
        );
        s.enabled = false;
        assert_eq!(
            follow.tick(true, &s, true, false, false),
            Some(Action::Leave)
        );
    }
}
