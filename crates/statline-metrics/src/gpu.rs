//! GPU sampling with lazy provider opening and a slower cadence than the strip.
//!
//! The PDH wildcard counters cost ~0.6 ms per collection because there is one
//! instance per engine per process. Sampling them once a second would spend a
//! quarter of the whole sampling budget on GPU alone, so the value is cached
//! and refreshed every three seconds. Vendor telemetry (NVML/ADL) rides the
//! same cadence and is entirely optional.

use statline_core::model::GpuSample;
use statline_win::metrics::gpu::GpuCounters;
use statline_win::metrics::gpu_vendor::VendorSensors;
use std::time::{Duration, Instant};

/// How often the GPU counters are actually read.
const REFRESH: Duration = Duration::from_secs(3);
/// Retry opening a provider at most this often when it is unavailable.
const RETRY: Duration = Duration::from_secs(60);

pub struct GpuState {
    counters: Option<GpuCounters>,
    tried_at: Option<Instant>,
    unavailable: bool,
    vendor: Option<VendorSensors>,
    vendor_tried_at: Option<Instant>,
    vendor_unavailable: bool,
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
            vendor: None,
            vendor_tried_at: None,
            vendor_unavailable: false,
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
            if !self.unavailable || retry {
                self.tried_at = Some(now);
                self.counters = GpuCounters::open();
                self.unavailable = self.counters.is_none();
            }
        }

        if self.vendor.is_none() {
            let retry = self
                .vendor_tried_at
                .map(|at| now.duration_since(at) >= RETRY)
                .unwrap_or(true);
            if !self.vendor_unavailable || retry {
                self.vendor_tried_at = Some(now);
                self.vendor = VendorSensors::open();
                self.vendor_unavailable = self.vendor.is_none();
            }
        }

        let counters = self
            .counters
            .as_ref()
            .and_then(|counters| counters.sample());
        let vendor = self.vendor.as_mut().and_then(|vendor| vendor.read());
        if counters.is_none() && vendor.is_none() {
            return None;
        }

        let counters = counters.unwrap_or_default();
        let vendor = vendor.unwrap_or_default();
        let utilization = counters
            .utilization_percent
            .max(vendor.utilization_percent.unwrap_or(0.0).clamp(0.0, 100.0));

        let sample = GpuSample {
            utilization_percent: utilization,
            vram_used_bytes: counters.vram_used_bytes,
            vram_total_bytes: counters.vram_total_bytes,
            name: vendor.name,
            temperature_c: vendor.temperature_c,
            fan_percent: vendor.fan_percent,
            fan_rpm: vendor.fan_rpm,
            power_w: vendor.power_w,
            core_mhz: vendor.core_mhz,
            memory_mhz: vendor.memory_mhz,
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
