use crate::sources::SourceInfo;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

pub const MIN_BITRATE: u64 = 1_000_000;
pub const MAX_BITRATE: u64 = 100_000_000;
pub const BITRATE_PRESETS: [u64; 11] = [6, 8, 10, 12, 16, 20, 25, 35, 50, 75, 100];
pub const FRAME_RATES: [u32; 6] = [15, 24, 30, 60, 90, 120];
const AUTO_CEILING: u64 = 35_000_000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    Av1,
    H265,
    H264,
}
impl Codec {
    pub fn label(self) -> &'static str {
        match self {
            Self::Av1 => "AV1",
            Self::H265 => "H.265 / HEVC",
            Self::H264 => "H.264 compatibility",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Resolution {
    P720,
    P1080,
    P1440,
    P2160,
    #[default]
    Source,
}
impl Resolution {
    pub const ALL: [Self; 5] = [
        Self::P720,
        Self::P1080,
        Self::P1440,
        Self::P2160,
        Self::Source,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::P720 => "720p",
            Self::P1080 => "1080p",
            Self::P1440 => "1440p",
            Self::P2160 => "4K",
            Self::Source => "Source · up to 4K",
        }
    }
    fn bounds(self) -> (u32, u32) {
        match self {
            Self::P720 => (1280, 720),
            Self::P1080 => (1920, 1080),
            Self::P1440 => (2560, 1440),
            Self::P2160 | Self::Source => (3840, 2160),
        }
    }
    pub fn dimensions(self, source: &SourceInfo) -> Result<(u32, u32)> {
        let (long, short) = self.bounds();
        let (bound_w, bound_h) = if source.height > source.width {
            (short, long)
        } else {
            (long, short)
        };
        ensure!(
            source.width > 0 && source.height > 0,
            "Refresh and select an available source."
        );
        ensure!(
            self == Self::Source || (source.width >= bound_w && source.height >= bound_h),
            "The selected resolution exceeds this source. Choose a smaller resolution or Source."
        );
        let scale = (bound_w as f64 / source.width as f64)
            .min(bound_h as f64 / source.height as f64)
            .min(1.0);
        let width = (source.width as f64 * scale) as u32 / 2 * 2;
        let height = (source.height as f64 * scale) as u32 / 2 * 2;
        ensure!(
            width >= 144 && height >= 128,
            "Choose a larger source window."
        );
        Ok((width, height))
    }
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    #[default]
    Motion,
    Detail,
    Saver,
}
impl Profile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Motion => "Games / motion",
            Self::Detail => "Text / desktop",
            Self::Saver => "Data saver",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "mode",
    content = "bps",
    rename_all = "lowercase",
    deny_unknown_fields
)]
pub enum Bitrate {
    #[default]
    Auto,
    Manual(u64),
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub resolution: Resolution,
    pub fps: u32,
    pub bitrate: Bitrate,
    pub profile: Profile,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            resolution: Resolution::Source,
            fps: 60,
            bitrate: Bitrate::Auto,
            profile: Profile::Motion,
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u64,
}
impl Settings {
    pub fn resolve(self, source: &SourceInfo, codec: Codec) -> Result<Mode> {
        let (width, height) = self.resolution.dimensions(source)?;
        Ok(Mode {
            width,
            height,
            fps: self.fps,
            bitrate: self.bitrate_for(width, height, codec)?,
        })
    }
    pub fn bitrate_for(self, width: u32, height: u32, codec: Codec) -> Result<u64> {
        ensure!(
            FRAME_RATES.contains(&self.fps),
            "Choose a supported frame rate up to 120 fps."
        );
        let bitrate = match self.bitrate {
            Bitrate::Manual(value) => {
                ensure!(
                    (MIN_BITRATE..=MAX_BITRATE).contains(&value),
                    "Choose a bitrate from 1 to 100 Mbps."
                );
                value
            }
            Bitrate::Auto => {
                // Lower-bandwidth starting targets, not a universal quality law.
                // See docs/bitrate-guidance.md for sources and our policy choices.
                let anchors = match codec {
                    // Use the HEVC targets until AV1 quality is measured locally.
                    Codec::Av1 | Codec::H265 => [
                        (1280 * 720, 2.5, 4.0),
                        (1920 * 1080, 4.0, 6.0),
                        (2560 * 1440, 6.0, 10.0),
                        (3840 * 2160, 12.0, 18.0),
                    ],
                    Codec::H264 => [
                        (1280 * 720, 3.0, 5.0),
                        (1920 * 1080, 5.0, 8.0),
                        (2560 * 1440, 8.0, 14.0),
                        (3840 * 2160, 16.0, 25.0),
                    ],
                };
                let pixels = width as f64 * height as f64;
                let mut lower = (0, 0.0, 0.0);
                let mut rates = (anchors[3].1, anchors[3].2);
                for upper in anchors {
                    if pixels <= upper.0 as f64 {
                        let fraction = ((pixels - lower.0 as f64) / (upper.0 - lower.0) as f64)
                            .clamp(0.0, 1.0);
                        rates = (
                            lower.1 + (upper.1 - lower.1) * fraction,
                            lower.2 + (upper.2 - lower.2) * fraction,
                        );
                        break;
                    }
                    lower = upper;
                }
                let mbps = if self.fps <= 30 {
                    rates.0 * self.fps as f64 / 30.0
                } else if self.fps <= 60 {
                    rates.0 + (rates.1 - rates.0) * (self.fps - 30) as f64 / 30.0
                } else {
                    rates.1 * self.fps as f64 / 60.0
                };
                let weight = match self.profile {
                    Profile::Motion => 1.0,
                    Profile::Detail => 0.9,
                    Profile::Saver => 0.65,
                };
                ((mbps * weight * 2.0).ceil() as u64 * 500_000).clamp(MIN_BITRATE, AUTO_CEILING)
            }
        };
        Ok(bitrate)
    }
}
impl Mode {
    pub fn baseline(source: &SourceInfo) -> Result<Self> {
        let scale = (1920.0 / source.width as f64)
            .min(1080.0 / source.height as f64)
            .min(1.0);
        let width = (source.width as f64 * scale) as u32 / 2 * 2;
        let height = (source.height as f64 * scale) as u32 / 2 * 2;
        ensure!(
            width >= 144 && height >= 128,
            "Choose a larger source window."
        );
        Ok(Self {
            width,
            height,
            fps: 60,
            bitrate: 12_000_000,
        })
    }
}

/// Live bitrate edits and congestion feedback share one cap; dimensions/FPS never adapt silently.
#[derive(Clone)]
pub struct RateControl {
    pub diagnostic_enabled: Arc<AtomicBool>,
    pub capture_frames: Arc<AtomicU64>,
    pub capture_overruns: Arc<AtomicU64>,
    pub encoder_inputs: Arc<AtomicU64>,
    requested: Arc<AtomicU64>,
    viewer_cap: Arc<AtomicU64>,
    upload_cap: Arc<AtomicU64>,
}
impl RateControl {
    pub fn new(requested: u64) -> Self {
        Self {
            diagnostic_enabled: Arc::default(),
            capture_frames: Arc::default(),
            capture_overruns: Arc::default(),
            encoder_inputs: Arc::default(),
            requested: Arc::new(AtomicU64::new(requested)),
            viewer_cap: Arc::new(AtomicU64::new(MAX_BITRATE)),
            upload_cap: Arc::new(AtomicU64::new(MAX_BITRATE)),
        }
    }
    pub fn requested(&self) -> u64 {
        self.requested.load(Ordering::Acquire)
    }
    pub fn set(&self, bitrate: u64) -> Result<()> {
        ensure!(
            (MIN_BITRATE..=MAX_BITRATE).contains(&bitrate),
            "Choose a bitrate from 1 to 100 Mbps."
        );
        self.requested.store(bitrate, Ordering::Release);
        Ok(())
    }
    pub fn set_viewer_cap(&self, cap: Option<u64>) {
        self.viewer_cap.store(
            cap.unwrap_or(MAX_BITRATE).clamp(64_000, MAX_BITRATE),
            Ordering::Release,
        );
    }
    pub fn effective(&self, feedback: Option<u64>) -> u64 {
        self.requested()
            .min(feedback.unwrap_or(MAX_BITRATE))
            .min(self.viewer_cap.load(Ordering::Acquire))
            .min(self.upload_cap.load(Ordering::Acquire))
            .max(1_000)
    }
    pub fn set_upload_cap(&self, cap: Option<u64>) {
        self.upload_cap.store(
            cap.unwrap_or(MAX_BITRATE).clamp(64_000, MAX_BITRATE),
            Ordering::Release,
        );
    }
    pub fn viewer_cap(&self) -> u64 {
        self.viewer_cap.load(Ordering::Acquire)
    }
    pub fn upload_cap(&self) -> u64 {
        self.upload_cap.load(Ordering::Acquire)
    }
}

/// A running single-layer stream cannot abruptly multiply its traffic and
/// expect every subscriber's bandwidth estimator to have followed already.
pub struct EncoderBudget {
    current: u64,
    last_requested: u64,
    ramping: bool,
}
impl EncoderBudget {
    pub fn new(requested: u64) -> Self {
        Self {
            current: requested,
            last_requested: requested,
            ramping: false,
        }
    }
    pub fn update(&mut self, rate: &RateControl, feedback: Option<u64>) -> u64 {
        let requested = rate.requested();
        if requested > self.last_requested {
            self.ramping = true;
        } else if requested < self.last_requested {
            self.ramping = false;
        }
        let wanted = rate.effective(feedback);
        if wanted > self.current {
            self.ramping = true;
        }
        self.current = if self.ramping && wanted > self.current {
            wanted.min(self.current.saturating_add((self.current / 4).max(500_000)))
        } else {
            wanted
        };
        if self.current >= requested {
            self.ramping = false;
        }
        self.last_requested = requested;
        self.current
    }
}

/// AMF 1.28.8 reopens its encoder component for bitrate property changes.
/// Keep reductions prompt, but avoid resetting it for every small BWE fluctuation.
pub struct ReinitializingBudget {
    current: u64,
    last_requested: u64,
    last_change: std::time::Duration,
}
impl ReinitializingBudget {
    pub fn new(current: u64, requested: u64) -> Self {
        Self {
            current,
            last_requested: requested,
            last_change: std::time::Duration::ZERO,
        }
    }
    pub fn update(
        &mut self,
        rate: &RateControl,
        feedback: Option<u64>,
        now: std::time::Duration,
    ) -> u64 {
        let requested = rate.requested();
        let explicit = requested != self.last_requested;
        self.last_requested = requested;
        let wanted = rate.effective(feedback);
        let next = if wanted < self.current {
            // Additional margin prevents repeated resets for nearby feedback.
            if explicit {
                wanted
            } else {
                (wanted * 95 / 100 / 1000 * 1000).max(1000)
            }
        } else if wanted > self.current
            && (explicit
                || (now.saturating_sub(self.last_change) >= std::time::Duration::from_secs(5)
                    && (wanted == requested
                        || wanted >= self.current.saturating_add((self.current / 10).max(64_000)))))
        {
            wanted.min(self.current.saturating_add((self.current / 4).max(500_000)))
        } else {
            self.current
        };
        if next != self.current {
            self.current = next;
            self.last_change = now;
        }
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reinitializing_encoder_avoids_noise_but_never_delays_a_lower_cap() {
        use std::time::Duration;
        let rate = RateControl::new(25_000_000);
        let mut budget = ReinitializingBudget::new(25_000_000, 25_000_000);
        let first = budget.update(&rate, Some(24_000_000), Duration::from_secs(1));
        assert!(first <= 24_000_000);
        for second in 2..20 {
            assert_eq!(
                budget.update(
                    &rate,
                    Some(23_900_000 + (second % 2) * 100_000),
                    Duration::from_secs(second)
                ),
                first
            );
        }
        rate.set_viewer_cap(Some(2_000_000));
        assert!(budget.update(&rate, None, Duration::from_secs(20)) <= 2_000_000);
        let low = budget.current;
        rate.set_viewer_cap(None);
        assert_eq!(budget.update(&rate, None, Duration::from_secs(21)), low);
        let up = budget.update(&rate, None, Duration::from_secs(25));
        assert!(up > low && up <= low + (low / 4).max(500_000));
        rate.set(1_000_000).unwrap();
        assert_eq!(
            budget.update(&rate, None, Duration::from_secs(26)),
            1_000_000
        );
    }

    #[test]
    fn watcher_departure_never_bypasses_upload_or_transport_limits() {
        let rate = RateControl::new(8_000_000);
        rate.set_upload_cap(Some(3_000_000));
        rate.set_viewer_cap(Some(1_000_000));
        assert_eq!(rate.effective(None), 1_000_000);
        rate.set_viewer_cap(None);
        assert_eq!(rate.effective(None), 3_000_000);
        assert_eq!(rate.effective(Some(2_000_000)), 2_000_000);
        assert_eq!(rate.requested(), 8_000_000);
        rate.set_upload_cap(None);
        assert_eq!(rate.effective(None), 8_000_000);
    }

    #[test]
    fn default_source_and_sensible_hd_targets_preserve_manual_and_watcher_caps() {
        let settings = Settings::default();
        assert_eq!(settings.resolution, Resolution::Source);
        assert_eq!(settings.fps, 60);
        assert_eq!(
            settings.bitrate_for(1920, 1080, Codec::H265).unwrap(),
            6_000_000
        );
        assert_eq!(
            settings.bitrate_for(1920, 1080, Codec::H264).unwrap(),
            8_000_000
        );
        let rate = RateControl::new(8_000_000);
        rate.set_viewer_cap(Some(2_000_000));
        assert_eq!(rate.effective(Some(10_000_000)), 2_000_000);
        assert_eq!(rate.requested(), 8_000_000);
        rate.set_viewer_cap(None);
        assert_eq!(rate.effective(None), 8_000_000);
    }

    #[test]
    fn live_increases_ramp_without_overshoot_and_reductions_apply_immediately() {
        let rate = RateControl::new(3_000_000);
        let mut budget = EncoderBudget::new(rate.requested());
        rate.set(25_000_000).unwrap();
        assert_eq!(budget.update(&rate, None), 3_750_000);
        let mut previous = 3_750_000;
        for _ in 0..12 {
            let next = budget.update(&rate, None);
            assert!(next >= previous && next <= 25_000_000);
            previous = next;
        }
        assert_eq!(previous, 25_000_000);
        rate.set(3_000_000).unwrap();
        assert_eq!(budget.update(&rate, None), 3_000_000);
        assert_eq!(budget.update(&rate, Some(1_000_000)), 1_000_000);
        assert_eq!(rate.requested(), 3_000_000);
    }
    fn source(width: u32, height: u32) -> SourceInfo {
        SourceInfo {
            id: "fixture".into(),
            label: "Synthetic".into(),
            width,
            height,
        }
    }
    #[test]
    fn dimensions_preserve_aspect_source_bounds_orientation_and_4k_ceiling() {
        assert_eq!(
            Resolution::P1080.dimensions(&source(3440, 1440)).unwrap(),
            (1920, 802)
        );
        assert_eq!(
            Resolution::Source.dimensions(&source(7680, 4320)).unwrap(),
            (3840, 2160)
        );
        assert_eq!(
            Resolution::Source.dimensions(&source(2160, 3840)).unwrap(),
            (2160, 3840)
        );
        assert!(Resolution::P2160.dimensions(&source(1920, 1080)).is_err());
        assert_eq!(
            Resolution::Source.dimensions(&source(1365, 767)).unwrap(),
            (1364, 766)
        );
    }
    #[test]
    fn auto_tracks_resolution_fps_codec_profile_and_manual_preserves_geometry() {
        let mut settings = Settings {
            resolution: Resolution::P1080,
            ..Settings::default()
        };
        let display = source(1920, 1080);
        let initial = settings.resolve(&display, Codec::H265).unwrap();
        settings.fps = 90;
        let faster = settings.resolve(&display, Codec::H265).unwrap();
        assert!(faster.bitrate > initial.bitrate);
        assert!(settings.resolve(&display, Codec::H264).unwrap().bitrate > faster.bitrate);
        settings.profile = Profile::Detail;
        assert!(settings.resolve(&display, Codec::H265).unwrap().bitrate < faster.bitrate);
        settings.bitrate = Bitrate::Manual(100_000_000);
        let manual = settings.resolve(&display, Codec::H265).unwrap();
        assert_eq!(
            (manual.width, manual.height, manual.fps),
            (faster.width, faster.height, faster.fps)
        );
        assert_eq!(manual.bitrate, MAX_BITRATE);
        settings.bitrate = Bitrate::Manual(0);
        assert!(settings.resolve(&display, Codec::H265).is_err());
        settings.fps = 121;
        assert!(settings.resolve(&display, Codec::H265).is_err());
    }
    #[test]
    fn congestion_caps_both_auto_and_manual_and_live_edits_do_not_rewrite_request() {
        let rate = RateControl::new(12_000_000);
        assert_eq!(rate.effective(Some(2_000_000)), 2_000_000);
        assert_eq!(rate.requested(), 12_000_000);
        rate.set(100_000_000).unwrap();
        assert_eq!(rate.effective(Some(150_000_000)), MAX_BITRATE);
        assert_eq!(rate.effective(Some(0)), 1_000);
        assert!(rate.set(MAX_BITRATE + 1).is_err());
    }
}
