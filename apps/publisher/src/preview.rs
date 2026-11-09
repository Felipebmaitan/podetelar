use crate::admission::{Admission, PreviewDelivery};
use livekit::prelude::LocalVideoTrack;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

pub const INTERVAL: Duration = Duration::from_secs(60);
pub const MAX_BYTES: usize = 65_536;
#[derive(Clone)]
pub struct Frame {
    pub sequence: u64,
    pub bytes: Vec<u8>,
}
pub type Latest = Arc<Mutex<Option<Frame>>>;

/// Runs on the tiny preview branch before GPU scaling, download and JPEG encode.
/// A reconnect/viewer/network retry cannot advance this capture clock.
pub struct Gate {
    started: Instant,
    next_ms: AtomicU64,
}
impl Gate {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            next_ms: AtomicU64::new(0),
        }
    }
    pub fn take(&self) -> bool {
        self.take_at(self.started.elapsed().as_millis().min(u64::MAX as u128) as u64)
    }
    fn take_at(&self, elapsed: u64) -> bool {
        let next = self.next_ms.load(Ordering::Acquire);
        elapsed >= next
            && self
                .next_ms
                .compare_exchange(
                    next,
                    elapsed.saturating_add(INTERVAL.as_millis() as u64),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
    }
}

async fn cancelled(stop: &AtomicBool) {
    while !stop.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Owns a single cached JPEG, independent of the video pump and RTC subscriptions.
pub async fn upload(
    admission: Admission,
    frames: Latest,
    video: LocalVideoTrack,
    stop: Arc<AtomicBool>,
    uploads: Arc<AtomicU64>,
) {
    let mut sent = (0, String::new());
    let mut attempted_sid = String::new();
    let mut retry = Instant::now();
    loop {
        if stop.load(Ordering::Acquire) {
            break;
        }
        let sid = video.sid().to_string();
        let frame = if Instant::now() >= retry {
            frames.lock().ok().and_then(|value| {
                value
                    .as_ref()
                    .filter(|frame| sent.0 != frame.sequence || sent.1 != sid)
                    .cloned()
            })
        } else {
            None
        };
        if let Some(frame) = frame
            && !sid.is_empty()
        {
            attempted_sid = sid.clone();
            let result = tokio::select! {
                _ = cancelled(&stop) => break,
                result = admission.send_preview(&sid, frame.bytes) => result,
            };
            match result {
                PreviewDelivery::Sent => {
                    sent = (frame.sequence, sid);
                    uploads.fetch_add(1, Ordering::Relaxed);
                }
                PreviewDelivery::Retry(delay) => retry = Instant::now() + delay,
                PreviewDelivery::Denied => break,
            }
        }
        tokio::select! { _ = cancelled(&stop) => break, _ = tokio::time::sleep(Duration::from_secs(1)) => {} }
    }
    let mut sids = vec![sent.1, attempted_sid, video.sid().to_string()];
    sids.sort();
    sids.dedup();
    sids.retain(|sid| !sid.is_empty());
    futures_util::future::join_all(sids.iter().map(|sid| admission.remove_preview(sid))).await;
    if let Ok(mut latest) = frames.lock() {
        *latest = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_clock_allows_start_then_at_most_one_frame_each_minute() {
        let gate = Gate::new();
        assert!(gate.take_at(0));
        for time in [0, 1, 999, 59_999] {
            assert!(!gate.take_at(time));
        }
        assert!(gate.take_at(60_000));
        assert!(!gate.take_at(60_000));
        // Delay moves the next capture forward; never catch up with a burst.
        assert!(gate.take_at(180_000));
        assert!(!gate.take_at(180_001));
        assert!(!gate.take_at(239_999));
        assert!(gate.take_at(240_000));
    }
}
