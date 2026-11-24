use crate::quality::{self, Bitrate, Codec, Mode, Settings, StreamPolicy};
use anyhow::{Result, ensure};
use serde::Serialize;

pub const MIN_BITRATE: u64 = 500_000;
pub const MAX_BITRATE: u64 = 8_000_000;
pub const FRAME_RATES: [u32; 3] = [24, 30, 60];
pub const TARGET_FRAME_RATES: [u32; 4] = [15, 24, 30, 60];
pub const TARGET_TIERS: [u32; 5] = [480, 720, 1080, 1440, 2160];
pub const RESOLUTIONS: [quality::Resolution; 3] = [
    quality::Resolution::P720,
    quality::Resolution::P1080,
    quality::Resolution::Source,
];

fn target_tier(width: u32, height: u32) -> u32 {
    let pixels = u64::from(width) * u64::from(height);
    match pixels {
        0..=307_200 => 480,
        307_201..=921_600 => 720,
        921_601..=2_073_600 => 1080,
        2_073_601..=3_686_400 => 1440,
        _ => 2160,
    }
}

fn automatic_bitrate(tier: u32, fps: u32, codec: Codec) -> u64 {
    // H.264 targets in 100 kbps increments for 24, 30, and 60 FPS.
    let h264 = match tier {
        480 => [6, 8, 12],
        720 => [10, 14, 22],
        1080 => [16, 22, 34],
        1440 => [25, 33, 50],
        _ => [40, 50, 70],
    };
    let index = if fps <= 24 {
        0
    } else if fps <= 30 {
        1
    } else {
        2
    };
    let percent = match codec {
        Codec::H264 => 100,
        Codec::H265 => 80,
        Codec::Av1 => 65,
    };
    let rounded = (h264[index] * percent + 50) / 100;
    (rounded * 100_000).max(MIN_BITRATE)
}

#[derive(Clone, Serialize)]
pub struct Info {
    pub id: String,
    pub name: String,
}

#[derive(Clone)]
pub struct Device {
    pub info: Info,
    #[cfg(windows)]
    device: gstreamer::Device,
}

pub fn available() -> bool {
    #[cfg(windows)]
    {
        gstreamer::ElementFactory::find("mfvideosrc").is_some()
    }
    #[cfg(not(windows))]
    false
}

pub fn enumerate() -> Vec<Device> {
    #[cfg(windows)]
    {
        use gstreamer::{self as gst, prelude::*};
        use sha2::{Digest, Sha256};
        use std::collections::HashSet;

        // This provider includes its own DirectShow fallback for devices such as
        // OBS Virtual Camera. Query it directly to exclude network capture inputs.
        let Some(provider) = gst::DeviceProviderFactory::by_name("mfdeviceprovider") else {
            return Vec::new();
        };
        let mut seen = HashSet::new();
        let mut devices: Vec<_> = provider
            .devices()
            .into_iter()
            .filter_map(|device| {
                let props = device.properties()?;
                let path = props.get::<String>("device.path").ok()?;
                let id = format!("{:x}", Sha256::digest(path.to_ascii_lowercase()));
                if path.is_empty() || !seen.insert(id.clone()) {
                    return None;
                }
                Some(Device {
                    info: Info {
                        id,
                        name: device.display_name().into(),
                    },
                    device,
                })
            })
            .collect();
        devices
            .sort_by_cached_key(|device| (device.info.name.to_lowercase(), device.info.id.clone()));
        devices
    }
    #[cfg(not(windows))]
    Vec::new()
}

#[cfg(windows)]
#[derive(Clone)]
pub struct Input {
    pub device: gstreamer::Device,
    pub caps: gstreamer::Caps,
}

pub fn validate_settings(quality: Settings) -> Result<()> {
    ensure!(
        FRAME_RATES.contains(&quality.fps),
        "Choose a camera frame rate from the list."
    );
    ensure!(
        RESOLUTIONS.contains(&quality.resolution),
        "Choose a camera resolution from the list."
    );
    if let Bitrate::Manual(bitrate) = quality.bitrate {
        ensure!(
            (MIN_BITRATE..=MAX_BITRATE).contains(&bitrate),
            "Choose a camera bitrate from 0.5 to 8 Mbps."
        );
    }
    Ok(())
}

pub fn mode(
    info: &super::SourceInfo,
    quality: Settings,
    codec: Codec,
    policy: &StreamPolicy,
) -> Result<Mode> {
    validate_settings(quality)?;
    let (width, height) = quality.resolution.dimensions(info)?;
    Ok(with_bitrate(
        policy,
        quality,
        codec,
        policy.limit_video(Mode {
            width,
            height,
            fps: quality.fps,
            bitrate: 0,
        })?,
    ))
}

pub fn with_bitrate(policy: &StreamPolicy, quality: Settings, codec: Codec, mode: Mode) -> Mode {
    let tier = target_tier(mode.width, mode.height);
    let bitrate = match quality.bitrate {
        Bitrate::Auto => policy
            .camera_target(tier, mode.fps)
            .unwrap_or_else(|| automatic_bitrate(tier, mode.fps, codec)),
        Bitrate::Manual(bitrate) => bitrate,
    };
    Mode {
        bitrate: bitrate.min(policy.camera_cap().unwrap_or(MAX_BITRATE)),
        ..mode
    }
}

impl Device {
    pub fn source(&self, quality: Settings) -> Result<super::Source> {
        #[cfg(windows)]
        {
            use anyhow::Context;
            use gstreamer::{self as gst, prelude::*};
            use std::cmp::Reverse;

            validate_settings(quality)?;
            let caps = self
                .device
                .caps()
                .context("Camera formats are unavailable. Refresh cameras and try again.")?;
            let minimum_fps = gst::Fraction::new(quality.fps as i32, 1);
            let candidates = caps.iter().filter_map(|format| {
                let jpeg = format.name() == "image/jpeg";
                if !jpeg && format.name() != "video/x-raw" {
                    return None;
                }
                let fps = format.get::<gst::Fraction>("framerate").ok()?;
                if fps < minimum_fps {
                    return None;
                }
                let width = u32::try_from(format.get::<i32>("width").ok()?).ok()?;
                let height = u32::try_from(format.get::<i32>("height").ok()?).ok()?;
                let info = super::SourceInfo {
                    id: self.info.id.clone(),
                    label: self.info.name.clone(),
                    width,
                    height,
                };
                let (output_width, output_height) = quality.resolution.dimensions(&info).ok()?;
                let output = u64::from(output_width) * u64::from(output_height);
                let input = u64::from(width) * u64::from(height);
                let rank = (Reverse(output), input, fps, jpeg);
                Some((rank, format.name(), fps, info))
            });
            let (_, media_type, fps, info) = candidates
                .min_by_key(|(rank, _, _, _)| *rank)
                .context("This camera does not offer the selected resolution and frame rate. Choose another mode or refresh cameras.")?;
            // OBS may change raw pixel format as its DirectShow filter connects.
            // Lock the chosen geometry and cadence; let the GPU upload renegotiate format.
            let caps = gst::Caps::builder(media_type)
                .field("width", info.width as i32)
                .field("height", info.height as i32)
                .field("framerate", fps)
                .build();
            Ok(super::Source {
                info,
                target: super::Target::Camera(Input {
                    device: self.device.clone(),
                    caps,
                }),
            })
        }
        #[cfg(not(windows))]
        {
            let _ = quality;
            anyhow::bail!("Camera sharing is available on Windows.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_camera_bitrate_uses_output_dimensions_fps_and_codec() {
        let info = super::super::SourceInfo {
            id: "example-camera".into(),
            label: "Camera".into(),
            width: 1920,
            height: 1080,
        };
        let mut settings = Settings {
            resolution: quality::Resolution::P720,
            fps: 30,
            bitrate: Bitrate::Auto,
            ..Default::default()
        };
        let policy = StreamPolicy::default();
        for (codec, target) in [
            (Codec::H264, 1_400_000),
            (Codec::H265, 1_100_000),
            (Codec::Av1, 900_000),
        ] {
            assert_eq!(
                mode(&info, settings, codec, &policy).unwrap().bitrate,
                target
            );
        }
        settings.resolution = quality::Resolution::Source;
        settings.fps = 60;
        assert_eq!(
            mode(&info, settings, Codec::H264, &policy).unwrap().bitrate,
            3_400_000
        );
        settings.bitrate = Bitrate::Manual(5_500_000);
        for codec in [Codec::H264, Codec::H265, Codec::Av1] {
            assert_eq!(
                mode(&info, settings, codec, &policy).unwrap().bitrate,
                5_500_000
            );
        }
    }

    #[test]
    fn camera_overrides_and_caps_use_effective_geometry_without_screen_targets() {
        let source = super::super::SourceInfo {
            id: "camera".into(),
            label: "Camera".into(),
            width: 1920,
            height: 1080,
        };
        let mut policy: StreamPolicy = serde_json::from_str(
            r#"{
            "maxBitrateBps": 1000000,
            "maxResolution": 720,
            "maxFps": 15,
            "automaticTargets":[{"resolution":720,"fps":15,"bitrateBps":4000000}],
            "cameraAutomaticTargets":[{"resolution":720,"fps":15,"bitrateBps":750000}]
        }"#,
        )
        .unwrap();
        policy.validate().unwrap();
        let settings = Settings {
            fps: 30,
            ..Settings::default()
        };
        let active = mode(&source, settings, Codec::H264, &policy).unwrap();
        assert_eq!(
            active,
            Mode {
                width: 1280,
                height: 720,
                fps: 15,
                bitrate: 750_000
            }
        );
        policy.max_camera_bitrate_bps = Some(500_000);
        assert_eq!(
            with_bitrate(&policy, settings, Codec::H264, active).bitrate,
            500_000
        );
        let manual = Settings {
            bitrate: Bitrate::Manual(2_000_000),
            ..settings
        };
        assert_eq!(
            with_bitrate(&policy, manual, Codec::H264, active).bitrate,
            500_000
        );
        policy.max_camera_bitrate_bps = Some(4_000_000);
        assert_eq!(
            with_bitrate(&policy, manual, Codec::H264, active).bitrate,
            1_000_000
        );
        assert_eq!(
            mode(
                &source,
                Settings {
                    fps: 24,
                    ..settings
                },
                Codec::Av1,
                &StreamPolicy::default()
            )
            .unwrap()
            .bitrate,
            1_000_000
        );
    }
}
