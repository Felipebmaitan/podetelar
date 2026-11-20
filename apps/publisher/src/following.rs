//! Voice discovery never starts capture or playback; the room worker owns teardown.
use crate::account::Credential;
use serde::Deserialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

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
pub struct Client {
    desired: Arc<Mutex<Desired>>,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
}
impl Client {
    pub fn new() -> Self {
        let desired = Arc::new(Mutex::new((0, None::<Credential>)));
        let state = Arc::new(Mutex::new(State::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (input, output, stopped) = (desired.clone(), state.clone(), stop.clone());
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
                        .redirect(reqwest::redirect::Policy::none())
                        .timeout(Duration::from_secs(5))
                        .build()
                    else {
                        return;
                    };
                    while !stopped.load(Ordering::Acquire) {
                        let (generation, credential) =
                            input.lock().map(|v| v.clone()).unwrap_or((0, None));
                        let result = if let Some(c) = credential {
                            async {
                                let mut response = c
                                    .authorize(http.get(format!("{}/api/auth/voice", c.server)))
                                    .send()
                                    .await
                                    .ok()?;
                                if !response.status().is_success() {
                                    return None;
                                }
                                let mut bytes = Vec::new();
                                while let Some(chunk) = response.chunk().await.ok()? {
                                    if bytes.len() + chunk.len() > 65536 {
                                        return None;
                                    }
                                    bytes.extend_from_slice(&chunk);
                                }
                                serde_json::from_slice::<State>(&bytes).ok()
                            }
                            .await
                        } else {
                            None
                        };
                        if let Ok(current) = input.lock()
                            && current.0 == generation
                            && let Ok(mut state) = output.lock()
                        {
                            *state = result.unwrap_or_default();
                        }
                        for _ in 0..20 {
                            if stopped.load(Ordering::Acquire) {
                                return;
                            }
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                });
            })
            .expect("Discord voice worker");
        Self {
            desired,
            state,
            stop,
        }
    }
    #[cfg(test)]
    pub fn disabled() -> Self {
        Self::fixture(State::default())
    }
    #[cfg(any(test, feature = "ui-review"))]
    pub fn fixture(state: State) -> Self {
        Self {
            desired: Arc::new(Mutex::new((0, None))),
            state: Arc::new(Mutex::new(state)),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn set_credential(&self, credential: Option<Credential>) {
        if let Ok(mut desired) = self.desired.lock() {
            desired.0 = desired.0.wrapping_add(1);
            desired.1 = credential;
            if let Ok(mut state) = self.state.lock() {
                *state = State::default();
            }
        }
    }
    pub fn state(&self) -> State {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
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
