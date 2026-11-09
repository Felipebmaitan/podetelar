use livekit::{RoomOptions, webrtc::peer_connection_factory::IceTransportsType};
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    #[default]
    Auto,
    Relay,
}

pub fn options(route: Route) -> RoomOptions {
    let mut options = RoomOptions::default();
    options.auto_subscribe = false;
    options.join_retries = 0;
    options.connect_timeout = Duration::from_secs(10);
    if route == Route::Relay {
        options.rtc_config.ice_transport_type = IceTransportsType::Relay;
    }
    options
}
