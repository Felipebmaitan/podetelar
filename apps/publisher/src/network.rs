//! A measured path capacity informs the allowance. Keep headroom, share it across actual
//! subscriptions, reduce on receiver evidence and recover slowly within that cap.
pub const DEFAULT_DOWNLOAD_BPS: u64 = 12_000_000;
pub const PROBE_CAP_BPS: u64 = 100_000_000;
pub const PROBE_BYTES: usize = 24 * 1024 * 1024;
#[derive(Clone, Default)]
pub struct Capacity {
    pub download_bps: Option<u64>,
    pub upload_bps: Option<u64>,
    pub error: String,
}
impl Capacity {
    pub fn watching_allowance(&self) -> Option<u64> {
        self.download_bps.map(|v| v.min(PROBE_CAP_BPS) * 4 / 5)
    }
    pub fn upload_video_cap(&self) -> Option<u64> {
        self.upload_bps
            .map(|v| (v * 4 / 5).saturating_sub(256_000).max(64_000))
    }
}

pub struct ReceiveBudget {
    allowance: u64,
    payload: u64,
    healthy: u8,
}

impl Default for ReceiveBudget {
    fn default() -> Self {
        Self {
            allowance: DEFAULT_DOWNLOAD_BPS,
            payload: DEFAULT_DOWNLOAD_BPS * 3 / 4,
            healthy: 0,
        }
    }
}

impl ReceiveBudget {
    pub fn set(&mut self, allowance: Option<u64>) {
        self.allowance = allowance
            .unwrap_or(DEFAULT_DOWNLOAD_BPS)
            .clamp(1, PROBE_CAP_BPS);
        self.payload = self.allowance * 3 / 4;
        self.healthy = 0;
    }

    pub fn observe(&mut self, loss: Option<f64>, buffer_seconds: Option<f64>) {
        // Optional incoming-bitrate stats are not a qualified capacity probe.
        // Keep one minimum subscription only when it fits the physical allowance.
        let ceiling = self.allowance * 3 / 4;
        let floor = 320_000.min(ceiling);
        self.payload = self.payload.min(ceiling).max(floor);
        if loss.is_some_and(|v| v > 0.03) || buffer_seconds.is_some_and(|v| v > 0.25) {
            self.payload = (self.payload * 3 / 4).max(floor);
            self.healthy = 0;
        } else if loss.is_some_and(|v| v < 0.01) && buffer_seconds.is_some_and(|v| v < 0.1) {
            self.healthy = self.healthy.saturating_add(1);
            if self.healthy >= 6 {
                self.payload = self
                    .payload
                    .saturating_add((self.payload / 20).max(64_000))
                    .min(ceiling);
                self.healthy = 0;
            }
        } else {
            self.healthy = 0;
        }
    }

    pub fn max_streams(&self) -> usize {
        (self.payload / (256_000 + 64_000)) as usize
    }

    pub fn per_stream(&self, streams: usize) -> u64 {
        let streams = streams.max(1) as u64;
        // Opus target is 192 kbps. Reserve 256 kbps per subscription inside the
        // payload allowance; the separate 25% margin covers overhead/bursts.
        self.payload
            .saturating_sub(streams * 256_000)
            .checked_div(streams)
            .unwrap_or(0)
            .clamp(64_000, 100_000_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_slow_links_do_not_get_inflated_into_a_watchable_budget() {
        let capacity = Capacity {
            download_bps: Some(200_000),
            upload_bps: Some(1_000_000),
            error: String::new(),
        };
        assert_eq!(capacity.watching_allowance(), Some(160_000));
        assert_eq!(capacity.upload_video_cap(), Some(544_000));
        let mut budget = ReceiveBudget::default();
        budget.set(capacity.watching_allowance());
        for _ in 0..100 {
            budget.observe(Some(0.1), Some(0.5));
        }
        assert_eq!(budget.max_streams(), 0);
        assert!(budget.payload <= 120_000);
    }

    #[test]
    fn several_watches_share_one_allowance_and_audio_headroom() {
        let mut budget = ReceiveBudget::default();
        budget.set(Some(5_000_000));
        let each = budget.per_stream(3);
        assert!(3 * (each + 256_000) <= 5_000_000 * 3 / 4);
        assert!(each < budget.per_stream(1));
    }

    #[test]
    fn loss_reduces_promptly_unknown_capacity_does_not_increase_and_recovery_is_bounded() {
        let mut budget = ReceiveBudget::default();
        let initial = budget.per_stream(1);
        budget.observe(Some(0.1), Some(0.3));
        let reduced = budget.per_stream(1);
        assert!(reduced < initial);
        for _ in 0..20 {
            budget.observe(None, None);
        }
        assert_eq!(budget.per_stream(1), reduced);
        for _ in 0..5 {
            budget.observe(Some(0.0), Some(0.02));
        }
        assert_eq!(budget.per_stream(1), reduced);
        budget.observe(Some(0.0), Some(0.02));
        assert!(budget.per_stream(1) > reduced);
        for _ in 0..1000 {
            budget.observe(Some(0.0), Some(0.02));
        }
        assert!(budget.per_stream(1) <= initial);
        for _ in 0..100 {
            budget.observe(Some(0.5), Some(0.3));
        }
        assert_eq!(budget.max_streams(), 1);
        assert_eq!(budget.per_stream(1), 64_000);
    }
}
