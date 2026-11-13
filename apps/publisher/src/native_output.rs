//! Shared output-only WebRTC rendering. Gain ownership survives device recovery.
use anyhow::{Result, ensure};
use livekit::{prelude::RemoteAudioTrack, rtc_engine::lk_runtime::LkRuntime};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use webrtc_sys::audio_device_controller::{
    SharedPtr,
    ffi::{AudioDeviceController, output_only_audit},
};

struct Device {
    controller: SharedPtr<AudioDeviceController>,
    users: usize,
    acquired: bool,
    rendering: bool,
    route: Option<String>,
    next_check: Instant,
    last_mix: u64,
    progress: Instant,
    recoveries: u64,
    failures: u64,
    route_changes: u64,
    held_until: Option<Instant>,
    test_route: Option<(String, Instant)>,
    _runtime: Arc<LkRuntime>,
}
static DEVICE: Mutex<Option<Device>> = Mutex::new(None);

impl Device {
    fn synthetic(&mut self) {
        self.controller.set_adm_playout_enabled(false);
        // InitPlayout failures can leave the proxy's logical playing flag false.
        // Keep receive decoding alive while Windows has no usable endpoint.
        if !self.controller.playout_is_initialized() {
            self.controller.init_playout();
        }
        self.controller.start_playout();
        self.rendering = false;
    }
    fn refresh(&mut self) -> bool {
        let now = Instant::now();
        if now < self.next_check {
            return self.rendering;
        }
        self.next_check = now + Duration::from_millis(500);
        let mix = output_only_audit().mix_calls;
        if mix != self.last_mix {
            self.last_mix = mix;
            self.progress = now;
        }
        let route = if self.held_until.is_some_and(|until| now < until) {
            Ok(None)
        } else if let Some((route, until)) = &self.test_route {
            if now < *until {
                Ok(Some(route.clone()))
            } else {
                self.test_route = None;
                default_route()
            }
        } else {
            default_route()
        };
        let route = match route {
            Ok(route) => route,
            Err(_)
                if self.rendering && now.duration_since(self.progress) < Duration::from_secs(3) =>
            {
                return true;
            }
            Err(_) => self.route.clone(),
        };
        if self.rendering
            && route == self.route
            && now.duration_since(self.progress) < Duration::from_secs(3)
        {
            return true;
        }
        if self.rendering {
            self.synthetic();
        }
        let Some(route) = route else {
            self.route = None;
            return false;
        };
        if !self.acquired {
            self.acquired = self.controller.acquire_platform_adm();
            if !self.acquired {
                self.failures += 1;
                self.synthetic();
                return false;
            }
        }
        // Select only this process's output. No Windows default setting changes.
        self.controller.stop_playout();
        let selected = self.controller.set_playout_device_by_guid(route.clone());
        self.controller.set_adm_playout_enabled(selected);
        if selected && self.controller.init_playout() && self.controller.start_playout() {
            if self.route.as_ref().is_some_and(|old| old != &route) {
                self.route_changes += 1;
            }
            self.route = Some(route);
            self.rendering = true;
            self.recoveries += 1;
            self.last_mix = output_only_audit().mix_calls;
            self.progress = Instant::now();
        } else {
            self.failures += 1;
            self.synthetic();
        }
        self.rendering
    }
}

#[cfg(windows)]
fn default_route() -> Result<Option<String>> {
    use windows::{
        Win32::{
            Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender},
            System::Com::{
                CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
                CoUninitialize,
            },
        },
        core::HRESULT,
    };
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _com = match unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() } {
        Ok(()) => Some(Com),
        Err(error) if error.code() == HRESULT(0x80010106_u32 as i32) => None,
        Err(error) => return Err(error.into()),
    };
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let endpoint = match unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) } {
        Ok(endpoint) => endpoint,
        Err(error) if error.code() == HRESULT(0x80070490_u32 as i32) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let id = unsafe { endpoint.GetId()? };
    let route = unsafe { id.to_string() };
    unsafe {
        CoTaskMemFree(Some(id.0.cast()));
    }
    Ok(Some(route?))
}
#[cfg(not(windows))]
fn default_route() -> Result<Option<String>> {
    Ok(None)
}

pub fn audit() -> serde_json::Value {
    let device = DEVICE.lock().ok();
    let active = device.as_ref().and_then(|slot| slot.as_ref());
    let audit = output_only_audit();
    serde_json::json!({
        "enabled": true, "blockedInputOperations":audit.blocked_input_operations,
        "platformOutputStarts":audit.platform_output_starts,"platformOutputStops":audit.platform_output_stops,
        "mixCalls":audit.mix_calls,
        "outputDelayMs":(audit.output_delay_ms >= 0).then_some(audit.output_delay_ms),
        "outputUsers":active.map_or(0, |d|d.users), "rendering":active.is_some_and(|d|d.rendering),
        "outputStarts":active.map_or(0, |d|d.recoveries),"outputFailures":active.map_or(0,|d|d.failures),
        "routeChanges":active.map_or(0,|d|d.route_changes),
        "availableOutputs":active.map_or(0,|d|d.controller.playout_devices()),
        "recordingEnabled":active.is_some_and(|d|d.controller.adm_recording_enabled()),
        "recordingInitialized":active.is_some_and(|d|d.controller.recording_is_initialized()),
    })
}

struct Lease;
impl Lease {
    fn acquire(track: &RemoteAudioTrack) -> Result<Self> {
        let mut slot = DEVICE
            .lock()
            .map_err(|_| anyhow::anyhow!("Audio device lock failed."))?;
        if let Some(device) = slot.as_mut() {
            device.users += 1;
            return Ok(Self);
        }
        let runtime = LkRuntime::instance();
        let controller = track.rtc_track().native_audio_handle().output_device();
        ensure!(
            !controller.is_null(),
            "Audio output controller unavailable."
        );
        controller.set_adm_recording_enabled(false);
        let now = Instant::now();
        let mut device = Device {
            controller,
            users: 1,
            acquired: false,
            rendering: false,
            route: None,
            next_check: now,
            last_mix: 0,
            progress: now,
            recoveries: 0,
            failures: 0,
            route_changes: 0,
            held_until: None,
            test_route: None,
            _runtime: runtime,
        };
        device.refresh();
        *slot = Some(device);
        Ok(Self)
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut slot) = DEVICE.lock() {
            let Some(device) = slot.as_mut() else { return };
            device.users -= 1;
            if device.users == 0 {
                device.controller.set_adm_playout_enabled(false);
                if device.acquired {
                    device.controller.release_platform_adm();
                }
                *slot = None;
            }
        }
    }
}
#[derive(PartialEq, Eq)]
pub enum OutputState {
    Playing,
    Waiting,
    Ended,
}
pub struct PlayingTrack {
    track: RemoteAudioTrack,
    generation: u64,
    _lease: Lease,
}
impl PlayingTrack {
    pub fn new(track: RemoteAudioTrack, generation: u64, percent: u32) -> Result<Option<Self>> {
        ensure!(generation != 0, "Remote audio mixer source unavailable.");
        // Reject a removed source before acquiring or restarting any device.
        if !track
            .rtc_track()
            .native_audio_handle()
            .set_playback_volume(generation, 0.0)
        {
            return Ok(None);
        }
        let lease = Lease::acquire(&track)?;
        let playing = Self {
            track,
            generation,
            _lease: lease,
        };
        // A source removed before this deferred task started is normal teardown.
        if !playing.set_volume(percent) {
            return Ok(None);
        }
        Ok(Some(playing))
    }
    fn set_volume(&self, percent: u32) -> bool {
        self.track
            .rtc_track()
            .native_audio_handle()
            .set_playback_volume(self.generation, percent.min(100) as f64 / 100.0)
    }
    pub fn refresh(&self, percent: u32) -> Result<OutputState> {
        if !self.set_volume(percent) {
            return Ok(OutputState::Ended);
        }
        let mut slot = DEVICE
            .lock()
            .map_err(|_| anyhow::anyhow!("Audio device lock failed."))?;
        Ok(if slot.as_mut().is_some_and(Device::refresh) {
            OutputState::Playing
        } else {
            OutputState::Waiting
        })
    }
}
impl Drop for PlayingTrack {
    fn drop(&mut self) {
        self.set_volume(0);
    }
}

// Explicit CLI qualification hooks. These affect this process's renderer only;
// they never change the user's Windows default or any other application's audio.
pub fn qualification_pause(seconds: u64) -> bool {
    let Ok(mut slot) = DEVICE.lock() else {
        return false;
    };
    let Some(device) = slot.as_mut() else {
        return false;
    };
    device.held_until = Some(Instant::now() + Duration::from_secs(seconds.clamp(1, 10)));
    device.next_check = Instant::now();
    device.refresh();
    true
}
pub fn qualification_switch(seconds: u64) -> bool {
    let Ok(mut slot) = DEVICE.lock() else {
        return false;
    };
    let Some(device) = slot.as_mut() else {
        return false;
    };
    let route = (0..device.controller.playout_devices().max(0) as u16)
        .map(|i| device.controller.playout_device_guid(i))
        .find(|id| !id.is_empty() && Some(id) != device.route.as_ref());
    let Some(route) = route else { return false };
    device.test_route = Some((
        route,
        Instant::now() + Duration::from_secs(seconds.clamp(1, 10)),
    ));
    device.next_check = Instant::now();
    device.refresh();
    true
}
