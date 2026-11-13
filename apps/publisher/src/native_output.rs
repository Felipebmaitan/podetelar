//! Pilot: one output-only WebRTC device, with independently owned remote gains.
use anyhow::{Context, Result, ensure};
use livekit::{prelude::RemoteAudioTrack, rtc_engine::lk_runtime::LkRuntime};
use std::sync::{Arc, Mutex};
use webrtc_sys::audio_device_controller::{SharedPtr, ffi::AudioDeviceController};

struct Device {
    controller: SharedPtr<AudioDeviceController>,
    users: usize,
    // The factory must outlive its ADM and every call made during teardown.
    _runtime: Arc<LkRuntime>,
}

static DEVICE: Mutex<Option<Device>> = Mutex::new(None);

pub fn enabled() -> bool {
    std::env::var("FREEDOMSTREAM_NATIVE_AUDIO").is_ok_and(|value| value == "1")
}

pub fn audit() -> serde_json::Value {
    let audit = webrtc_sys::audio_device_controller::ffi::output_only_audit();
    let device = DEVICE.lock().ok();
    let active = device.as_ref().and_then(|slot| slot.as_ref());
    serde_json::json!({
        "enabled": enabled(),
        "blockedInputOperations": audit.blocked_input_operations,
        "platformOutputStarts": audit.platform_output_starts,
        "platformOutputStops": audit.platform_output_stops,
        "outputDelayMs": (audit.output_delay_ms >= 0).then_some(audit.output_delay_ms),
        "outputUsers": active.map_or(0, |d| d.users),
        "recordingEnabled": active.is_some_and(|d| d.controller.adm_recording_enabled()),
        "recordingInitialized": active.is_some_and(|d| d.controller.recording_is_initialized()),
    })
}

struct Lease;
impl Lease {
    fn acquire(track: &RemoteAudioTrack) -> Result<Self> {
        // Serialize first acquire and last release, including their device calls.
        // A Weak owner alone permits last-drop/new-acquire to race each other.
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
        // Do not use PlatformAudio::new(): it opts into recording and speech DSP.
        controller.set_adm_recording_enabled(false);
        ensure!(
            controller.acquire_platform_adm(),
            "Windows audio output unavailable."
        );
        controller.set_adm_playout_enabled(true);
        if (!controller.playout_is_initialized() && !controller.init_playout())
            || !controller.start_playout()
        {
            controller.set_adm_playout_enabled(false);
            controller.release_platform_adm();
            anyhow::bail!("Windows audio output could not start.");
        }
        *slot = Some(Device {
            controller,
            users: 1,
            _runtime: runtime,
        });
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
                device.controller.release_platform_adm();
                *slot = None;
            }
        }
    }
}

pub struct PlayingTrack {
    track: RemoteAudioTrack,
    generation: u64,
    _lease: Lease,
}
impl PlayingTrack {
    pub fn new(track: RemoteAudioTrack, generation: u64, percent: u32) -> Result<Self> {
        let lease = Lease::acquire(&track)?;
        ensure!(generation != 0, "Remote audio mixer source unavailable.");
        let playing = Self {
            track,
            generation,
            _lease: lease,
        };
        playing.set_volume(percent)?;
        Ok(playing)
    }
    pub fn set_volume(&self, percent: u32) -> Result<()> {
        self.track
            .rtc_track()
            .native_audio_handle()
            .set_playback_volume(self.generation, percent.min(100) as f64 / 100.0)
            .then_some(())
            .context("Remote audio gain unavailable.")
    }
}
impl Drop for PlayingTrack {
    fn drop(&mut self) {
        // Silence this source before releasing a shared device used by others.
        let _ = self.set_volume(0);
    }
}
