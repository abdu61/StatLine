//! Battery sampling. Returns `None` on desktops, which is expected.
//!
//! Percent, AC state and rate are cheap and read every tick. Health and
//! temperature need a device handle and IOCTLs, so they are cached for 30 s and
//! retried once a minute when the firmware does not expose them.

use statline_core::model::BatterySample;
use statline_win::metrics::battery::{self, BatteryDetails};
use std::time::{Duration, Instant};

/// How often the battery device is queried for health and temperature.
const DETAILS_REFRESH: Duration = Duration::from_secs(30);
/// Retry a battery device that refused the IOCTLs at most this often.
const DETAILS_RETRY: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct BatteryState {
    details: Option<BatteryDetails>,
    details_at: Option<Instant>,
    tried_at: Option<Instant>,
    unavailable: bool,
}

impl BatteryState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Force the health/temperature cache to refresh on the next sample.
    pub fn set_details_visible(&mut self, visible: bool) {
        if visible {
            self.details_at = None;
        }
    }

    pub fn sample(&mut self, now: Instant, with_details: bool) -> Option<BatterySample> {
        let reading = battery::battery()?;
        if with_details {
            self.refresh_details(now);
        }
        let details = if with_details { self.details } else { None };

        Some(BatterySample {
            percent: reading.percent,
            on_ac: reading.on_ac,
            seconds_left: reading.seconds_left,
            rate_mw: reading.rate_mw,
            designed_mwh: details.and_then(|details| details.designed_mwh),
            full_mwh: details.and_then(|details| details.full_mwh),
            temperature_c: details.and_then(|details| details.temperature_c),
            cycle_count: details.and_then(|details| details.cycle_count),
        })
    }

    fn refresh_details(&mut self, now: Instant) {
        let stale = self
            .details_at
            .map(|at| now.duration_since(at) >= DETAILS_REFRESH)
            .unwrap_or(true);
        if !stale {
            return;
        }
        if self.unavailable {
            let retry = self
                .tried_at
                .map(|at| now.duration_since(at) >= DETAILS_RETRY)
                .unwrap_or(true);
            if !retry {
                return;
            }
        }

        self.tried_at = Some(now);
        match battery::details() {
            Some(details) => {
                self.details = Some(details);
                self.details_at = Some(now);
                self.unavailable = false;
            }
            None => self.unavailable = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_is_optional_but_valid_when_present() {
        let mut state = BatteryState::new();
        if let Some(battery) = state.sample(Instant::now(), true) {
            assert!((0.0..=100.0).contains(&battery.percent));
            if let Some(health) = battery.health_percent() {
                assert!((0.0..=200.0).contains(&health));
            }
            if let Some(temperature) = battery.temperature_c {
                assert!((-20.0..=80.0).contains(&temperature));
            }
        }
    }

    #[test]
    fn details_are_cached_between_samples() {
        let mut state = BatteryState::new();
        let now = Instant::now();
        let first = state.sample(now, true);
        let second = state.sample(now + Duration::from_secs(1), true);
        assert_eq!(first, second);
    }
}
