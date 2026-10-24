//! Disk sampling: PDH rates plus cached volume capacity.

use statline_core::model::{DiskSample, VolumeSample};
use statline_win::metrics::disk::{DiskCounters, volumes};
use std::time::{Duration, Instant};

/// Volume capacity changes slowly; querying it every tick is wasted work.
const VOLUME_REFRESH: Duration = Duration::from_secs(30);

pub struct DiskState {
    counters: Option<DiskCounters>,
    volumes: Vec<VolumeSample>,
    volumes_at: Option<Instant>,
}

impl Default for DiskState {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskState {
    pub fn new() -> Self {
        Self {
            counters: None,
            volumes: Vec::new(),
            volumes_at: None,
        }
    }

    /// Read rates and refresh capacity no more often than `VOLUME_REFRESH`.
    ///
    /// Returns `None` when PDH is unavailable; the caller hides the module
    /// rather than showing zeros.
    pub fn sample(&mut self, now: Instant) -> Option<DiskSample> {
        if self.counters.is_none() {
            self.counters = DiskCounters::open();
        }
        let rates = self.counters.as_ref()?.sample()?;

        let stale = self
            .volumes_at
            .map(|at| now.duration_since(at) >= VOLUME_REFRESH)
            .unwrap_or(true);
        if stale {
            self.volumes = volumes()
                .into_iter()
                .map(|volume| VolumeSample {
                    mount: volume.mount,
                    label: volume.label,
                    free_bytes: volume.free_bytes,
                    total_bytes: volume.total_bytes,
                    temperature_c: None,
                })
                .collect();
            self.volumes_at = Some(now);
        }

        Some(DiskSample {
            read_bytes_per_sec: rates.read_bytes_per_sec,
            write_bytes_per_sec: rates.write_bytes_per_sec,
            busy_percent: rates.busy_percent,
            volumes: self.volumes.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_reports_rates_and_volumes() {
        let mut state = DiskState::new();
        // PDH rate counters need two collections to produce a value.
        std::thread::sleep(Duration::from_millis(80));
        let sample = state.sample(Instant::now()).expect("disk sample");
        assert!(sample.read_bytes_per_sec >= 0.0);
        assert!(sample.write_bytes_per_sec >= 0.0);
        assert!((0.0..=100.0).contains(&sample.busy_percent));
        assert!(!sample.volumes.is_empty());
        assert!(sample.volumes.iter().all(|v| v.total_bytes > 0));
    }
}
