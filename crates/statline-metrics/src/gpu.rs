//! GPU sampling with lazy provider opening and a slower cadence than the strip.
//!
//! The PDH wildcard counters cost ~0.6 ms per collection because there is one
//! instance per engine per process. Sampling them once a second would spend a
//! quarter of the whole sampling budget on GPU alone, so the value is cached
//! and refreshed every three seconds.

use statline_core::model::GpuSample;
use statline_win::metrics::gpu::GpuCounters;
use std::time::{Duration, Instant};

/// How often the GPU counters are actually read.
const REFRESH: Duration = Duration::from_secs(3);
/// Retry opening the counters at most this often when they are unavailable.
const RETRY: Duration = Duration::from_secs(60);

pub struct GpuState {
    counters: Option<GpuCounters>,
    tried_at: Option<Instant>,
    unavailable: bool,
    cached: Option<GpuSample>,
    cached_at: Option<Instant>,
}

impl Default for GpuState {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuState {
    pub fn new() -> Self {
        Self {
            counters: None,
            tried_at: None,
            unavailable: false,
            cached: None,
            cached_at: None,
        }
    }

    pub fn sample(&mut self, now: Instant) -> Option<GpuSample> {
        let fresh = self
            .cached_at
            .map(|at| now.duration_since(at) < REFRESH)
            .unwrap_or(false);
        if fresh {
            return self.cached.clone();
        }

        if self.counters.is_none() {
            let retry = self
                .tried_at
                .map(|at| now.duration_since(at) >= RETRY)
                .unwrap_or(true);
            if self.unavailable && !retry {
                return None;
            }
            self.tried_at = Some(now);
            self.counters = GpuCounters::open();
            self.unavailable = self.counters.is_none();
        }

        let sample = self.counters.as_ref()?.sample()?;
        let sample = GpuSample {
            utilization_percent: sample.utilization_percent,
            vram_used_bytes: sample.vram_used_bytes,
            vram_total_bytes: sample.vram_total_bytes,
            temperature_c: None,
            fan_percent: None,
            power_w: None,
        };
        self.cached = Some(sample.clone());
        self.cached_at = Some(now);
        Some(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_never_panics_without_a_gpu() {
        let mut state = GpuState::new();
        if let Some(sample) = state.sample(Instant::now()) {
            assert!((0.0..=100.0).contains(&sample.utilization_percent));
        }
    }

    #[test]
    fn cached_values_are_reused_within_the_refresh_window() {
        let mut state = GpuState::new();
        let now = Instant::now();
        let first = state.sample(now);
        let second = state.sample(now + Duration::from_millis(200));
        assert_eq!(first, second, "a cached sample must not re-read counters");
    }
}
