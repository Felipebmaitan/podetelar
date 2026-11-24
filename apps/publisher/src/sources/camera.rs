use crate::quality::{self, Bitrate, Mode, Settings};
use anyhow::{Result, ensure};
use serde::Serialize;

pub const MIN_BITRATE: u64 = 500_000;
pub const MAX_BITRATE: u64 = 8_000_000;
pub const FRAME_RATES: [u32; 3] = [24, 30, 60];
pub const RESOLUTIONS: [quality::Resolution; 3] = [
    quality::Resolution::P720,
    quality::Resolution::P1080,
    quality::Resolution::Source,
];

fn automatic_bitrate(width: u32, height: u32, fps: u32) -> u64 {
    let pixels = u64::from(width) * u64::from(height);
    let targets = match pixels {
        0..=307_200 => [800_000, 1_000_000, 1_500_000],
        307_201..=921_600 => [1_500_000, 2_000_000, 3_000_000],
        921_601..=2_073_600 => [2_500_000, 3_000_000, 4_500_000],
        2_073_601..=3_686_400 => [3_500_000, 4_500_000, 6_000_000],
        _ => [5_000_000, 6_500_000, 8_000_000],
    };
    targets[if fps <= 24 {
        0
    } else if fps <= 30 {
        1
    } else {
        2
    }]
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

/// The bitrate is independent of the screen presets and room codec selection.
pub fn mode(info: &super::SourceInfo, quality: Settings) -> Result<Mode> {
    ensure!(
        FRAME_RATES.contains(&quality.fps),
        "Choose a camera frame rate from the list."
    );
    ensure!(
        RESOLUTIONS.contains(&quality.resolution),
        "Choose a camera resolution from the list."
    );
    let (width, height) = quality.resolution.dimensions(info)?;
    let bitrate = match quality.bitrate {
        Bitrate::Auto => automatic_bitrate(width, height, quality.fps),
        Bitrate::Manual(bitrate) => bitrate,
    };
    ensure!(
        (MIN_BITRATE..=MAX_BITRATE).contains(&bitrate),
        "Choose a camera bitrate from 0.5 to 8 Mbps."
    );
    Ok(Mode {
        width,
        height,
        fps: quality.fps,
        bitrate,
    })
}

impl Device {
    pub fn source(&self, quality: Settings) -> Result<super::Source> {
        #[cfg(windows)]
        {
            use anyhow::Context;
            use gstreamer::{self as gst, prelude::*};
            use std::cmp::Reverse;

            let caps = self
                .device
                .caps()
                .context("Camera formats are unavailable. Refresh cameras and try again.")?;
            ensure!(
                FRAME_RATES.contains(&quality.fps),
                "Choose a camera frame rate from the list."
            );
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
                let mode = mode(&info, quality).ok()?;
                let output = u64::from(mode.width) * u64::from(mode.height);
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
    fn automatic_camera_bitrate_uses_output_dimensions_and_fps() {
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
        assert_eq!(mode(&info, settings).unwrap().bitrate, 2_000_000);
        settings.resolution = quality::Resolution::Source;
        settings.fps = 60;
        assert_eq!(mode(&info, settings).unwrap().bitrate, 4_500_000);
        settings.bitrate = Bitrate::Manual(5_500_000);
        assert_eq!(mode(&info, settings).unwrap().bitrate, 5_500_000);
    }
}
