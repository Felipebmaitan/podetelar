//! Explicit qualification sender for pre-encoded AV1 IVF.
//! Excluded from shipped builds; there is no macOS AV1 encoder fallback.
use anyhow::{Context, Result, ensure};
use livekit::{
    options::{TrackPublishOptions, VideoCodec, VideoEncoderBackend, VideoEncoding},
    prelude::*,
    webrtc::{
        video_frame::{EncodedFrameType, EncodedVideoCodec, EncodedVideoFrame},
        video_source::{RtcVideoSource, VideoResolution, native::NativeVideoSource},
    },
};
use serde::Deserialize;
use std::{
    io::Read,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    invitation_url: String,
    fixture_path: PathBuf,
    seconds: u64,
    key_frames: Vec<usize>,
}

pub async fn publish(request: Request) -> Result<()> {
    ensure!(
        (10..=30).contains(&request.seconds),
        "Invalid fixture duration."
    );
    ensure!(
        crate::protection::unwrap_invitation(&request.invitation_url)?
            .1
            .is_none(),
        "The standalone IVF fixture requires a transport-only invitation."
    );
    let mut data = Vec::new();
    std::fs::File::open(request.fixture_path)?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut data)?;
    ensure!(
        data.len() >= 32 && data.len() <= 64 * 1024 * 1024,
        "Invalid IVF fixture size."
    );
    ensure!(
        &data[..4] == b"DKIF" && data[4..8] == [0, 0, 32, 0] && &data[8..12] == b"AV01",
        "Expected AV1 IVF."
    );
    let width = u16::from_le_bytes(data[12..14].try_into()?) as u32;
    let height = u16::from_le_bytes(data[14..16].try_into()?) as u32;
    let rate = u32::from_le_bytes(data[16..20].try_into()?);
    let scale = u32::from_le_bytes(data[20..24].try_into()?);
    ensure!(
        width >= 2
            && height >= 2
            && width <= 3840
            && height <= 3840
            && u64::from(width) * u64::from(height) <= 3840 * 2160
            && scale == 1
            && (1..=60).contains(&rate),
        "Invalid IVF mode."
    );
    let mut frames = Vec::new();
    let mut offset = 32;
    while offset < data.len() {
        ensure!(
            offset + 12 <= data.len() && frames.len() < 1800,
            "Invalid IVF frame header."
        );
        let length = u32::from_le_bytes(data[offset..offset + 4].try_into()?) as usize;
        offset += 12;
        ensure!(
            length > 0 && length <= 4 * 1024 * 1024 && offset + length <= data.len(),
            "Invalid IVF frame size."
        );
        frames.push(&data[offset..offset + length]);
        offset += length;
    }
    ensure!(
        frames.len() >= (request.seconds * u64::from(rate)) as usize,
        "Insufficient IVF frames."
    );
    ensure!(
        request.key_frames.first() == Some(&0)
            && request.key_frames.len() <= frames.len()
            && request.key_frames.windows(2).all(|pair| pair[0] < pair[1])
            && request
                .key_frames
                .last()
                .is_some_and(|last| *last < frames.len()),
        "Invalid fixture keyframe index."
    );
    let admission =
        crate::admission::Admission::join(&request.invitation_url, "AV1 qualification source")
            .await?;
    let result = async {
        let access = admission.media_access().await?;
        let (room, _events) = tokio::time::timeout(
            Duration::from_secs(15),
            Room::connect(
                &access.server_url,
                &access.token,
                crate::connection::options(crate::connection::Route::Auto),
            ),
        )
        .await
        .context("Fixture connection timed out.")?
        .map_err(|_| anyhow::anyhow!("Fixture connection failed."))?;
        let outcome = async {
            let mut capabilities = crate::playback::Playback::new();
            capabilities.reconcile(&room, &admission).await?;
            let resolution = VideoResolution { width, height };
            let source = NativeVideoSource::new_encoded(resolution.clone());
            let track = LocalVideoTrack::create_video_track(
                "PodeTelar screen",
                RtcVideoSource::Native(source.clone()),
            );
            tokio::time::timeout(
                Duration::from_secs(12),
                room.local_participant().publish_track(
                    LocalTrack::Video(track),
                    TrackPublishOptions {
                        video_codec: VideoCodec::AV1,
                        video_encoder: VideoEncoderBackend::PreEncoded,
                        simulcast: false,
                        source: TrackSource::Screenshare,
                        video_encoding: Some(VideoEncoding {
                            max_bitrate: 20_000_000,
                            max_framerate: f64::from(rate),
                        }),
                        ..Default::default()
                    },
                ),
            )
            .await
            .context("Fixture publication timed out.")?
            .map_err(|_| anyhow::anyhow!("Fixture publication failed."))?;
            let epoch = SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros() as i64;
            let mut interval =
                tokio::time::interval(Duration::from_nanos(1_000_000_000 / u64::from(rate)));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut accepted = 0;
            for (index, payload) in frames
                .iter()
                .take((request.seconds * u64::from(rate)) as usize)
                .enumerate()
            {
                interval.tick().await;
                accepted += usize::from(source.capture_encoded_frame(&EncodedVideoFrame {
                    codec: EncodedVideoCodec::AV1,
                    payload,
                    timestamp_us: epoch + index as i64 * 1_000_000 / i64::from(rate),
                    frame_type: if request.key_frames.binary_search(&index).is_ok() {
                        EncodedFrameType::Key
                    } else {
                        EncodedFrameType::Delta
                    },
                    resolution: resolution.clone(),
                    frame_metadata: None,
                }));
            }
            println!(
                "{}",
                serde_json::json!({"fixture":"preencoded-av1","accepted":accepted})
            );
            Ok::<_, anyhow::Error>(())
        }
        .await;
        let _ = room.close().await;
        outcome
    }
    .await;
    admission.leave().await;
    result
}
