use livekit::{prelude::LocalVideoTrack, webrtc::stats::RtcStats};
use serde::Serialize;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Default, Clone, Serialize)]
pub struct Metrics {
    pub available: bool,
    pub bitrate_bps: u64,
    pub retransmit_bps: u64,
    pub sent_fps: f64,
    pub rtt_ms: Option<f64>,
    pub loss_fraction: Option<f64>,
}

/// Samples aggregate outbound video facts once a second. Raw RTC stats never leave this module.
pub async fn monitor(video: LocalVideoTrack, stop: Arc<AtomicBool>, metrics: Arc<Mutex<Metrics>>) {
    let mut previous: Option<(String, Instant, u64, u64, u64)> = None;
    while !stop.load(Ordering::Acquire) {
        let result = tokio::time::timeout(Duration::from_millis(500), video.get_stats()).await;
        let mut sample = Metrics::default();
        if let Ok(Ok(stats)) = result {
            let (mut bytes, mut retransmits, mut frames) = (0_u64, 0_u64, 0_u64);
            for stat in stats {
                match stat {
                    RtcStats::OutboundRtp(stat) if stat.stream.kind == "video" => {
                        bytes = bytes.saturating_add(stat.sent.bytes_sent);
                        retransmits =
                            retransmits.saturating_add(stat.outbound.retransmitted_bytes_sent);
                        frames = frames.saturating_add(stat.outbound.frames_sent as u64);
                    }
                    RtcStats::RemoteInboundRtp(stat) if stat.stream.kind == "video" => {
                        sample.rtt_ms = Some(stat.remote_inbound.round_trip_time * 1000.0);
                        sample.loss_fraction = Some(stat.remote_inbound.fraction_lost);
                    }
                    _ => {}
                }
            }
            let sid = video.sid().to_string();
            let now = Instant::now();
            if let Some((last_sid, time, last_bytes, last_retransmits, last_frames)) = &previous
                && last_sid == &sid
                && bytes >= *last_bytes
                && frames >= *last_frames
            {
                let seconds = now.duration_since(*time).as_secs_f64().max(0.001);
                sample.available = true;
                sample.bitrate_bps = ((bytes - last_bytes) as f64 * 8.0 / seconds) as u64;
                sample.retransmit_bps =
                    (retransmits.saturating_sub(*last_retransmits) as f64 * 8.0 / seconds) as u64;
                sample.sent_fps = (frames - last_frames) as f64 / seconds;
            }
            previous = Some((sid, now, bytes, retransmits, frames));
        } else {
            previous = None;
        }
        if let Ok(mut state) = metrics.lock() {
            *state = sample;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
