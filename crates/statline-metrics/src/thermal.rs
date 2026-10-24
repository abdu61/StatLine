//! Thermal sampling: ACPI zones, refreshed slowly.
//!
//! PDH is not free and ACPI temperatures move slowly, so the query is sampled
//! every five seconds and cached in between. A missing counter hides the module.

use statline_win::metrics::thermal::ThermalCounters;
use std::time::{Duration, Instant};

/// How often the ACPI zones are actually read.
const REFRESH: Duration = Duration::from_secs(5);
/// Retry opening the query at most this often when it is unavailable.
const RETRY: Duration = Duration::from_secs(60);

pub struct ThermalState {
    counters: Option<ThermalCounters>,
    tried_at: Option<Instant>,
    unavailable: bool,
    cached: Option<f32>,
    cached_at: Option<Instant>,
}

impl Default for ThermalState {
    fn default() -> Self {
        Self::new()
    }
}

impl ThermalState {
    pub fn new() -> Self {
        Self {
            counters: None,
            tried_at: None,
            unavailable: false,
            cached: None,
            cached_at: None,
        }
    }

    /// Drop the cache so the next sample re-reads the zones.
    pub fn invalidate(&mut self) {
        self.cached_at = None;
    }

    /// Hottest valid zone in °C, refreshed on the thermal cadence.
    pub fn sample(&mut self, now: Instant) -> Option<f32> {
        let fresh = self
            .cached_at
            .map(|at| now.duration_since(at) < REFRESH)
            .unwrap_or(false);
        if fresh {
            return self.cached;
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
            self.counters = ThermalCounters::open();
            self.unavailable = self.counters.is_none();
        }

        // Cache the attempt even when the read has no valid zone: the query
        // costs ~1 ms, and a transient PDH gap must not run it every tick.
        let sample = self
            .counters
            .as_ref()
            .and_then(|counters| counters.sample());
        self.cached = sample;
        self.cached_at = Some(now);
        sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_reading_is_reused_within_the_window() {
        let mut state = ThermalState::new();
        let now = Instant::now();
        let first = state.sample(now);
        let second = state.sample(now + Duration::from_secs(1));
        assert_eq!(first, second);
    }

    #[test]
    fn reading_is_absent_or_plausible() {
        let mut state = ThermalState::new();
        if let Some(celsius) = state.sample(Instant::now()) {
            assert!((-20.0..=125.0).contains(&celsius));
        }
    }
}
