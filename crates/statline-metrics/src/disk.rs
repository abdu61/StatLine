//! Disk sampling: PDH rates plus cached volume capacity and temperatures.

use statline_core::model::{DiskSample, VolumeSample};
use statline_win::metrics::disk::{DiskCounters, volume_temperatures, volumes};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Volume capacity changes slowly; querying it every tick is wasted work.
const VOLUME_REFRESH: Duration = Duration::from_secs(30);
/// Drive sensors cost a volume IOCTL and a disk IOCTL (~11 ms here), and heat
/// moves slowly, so the reading is kept for five minutes.
const TEMPERATURE_REFRESH: Duration = Duration::from_secs(300);

pub struct DiskState {
    counters: Option<DiskCounters>,
    volumes: Vec<VolumeSample>,
    volumes_at: Option<Instant>,
    temperatures: HashMap<String, Option<f32>>,
    temperatures_at: Option<Instant>,
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
            temperatures: HashMap::new(),
            temperatures_at: None,
        }
    }

    /// Force the drive-temperature cache to refresh on the next sample.
    pub fn set_details_visible(&mut self, visible: bool) {
        if visible {
            self.temperatures_at = None;
        }
    }

    /// Read rates and refresh capacity no more often than `VOLUME_REFRESH`.
    ///
    /// `with_temperatures` is the detail-surface flag: the strip alone never
    /// needs the device IOCTLs. Returns `None` when PDH is unavailable; the
    /// caller hides the module rather than showing zeros.
    pub fn sample(&mut self, now: Instant, with_temperatures: bool) -> Option<DiskSample> {
        if self.counters.is_none() {
            self.counters = DiskCounters::open();
        }
        let rates = self.counters.as_ref()?.sample()?;

        let stale = self
            .volumes_at
            .map(|at| now.duration_since(at) >= VOLUME_REFRESH)
            .unwrap_or(true);
        if stale {
            if with_temperatures {
                let temperatures_stale = self
                    .temperatures_at
                    .map(|at| now.duration_since(at) >= TEMPERATURE_REFRESH)
                    .unwrap_or(true);
                if temperatures_stale {
                    self.temperatures = volume_temperatures().into_iter().collect();
                    self.temperatures_at = Some(now);
                }
            } else {
                // Nothing displays them; force a fresh read when a surface opens.
                self.temperatures_at = None;
                self.temperatures.clear();
            }

            self.volumes = volumes()
                .into_iter()
                .map(|volume| VolumeSample {
                    temperature_c: self.temperatures.get(&volume.mount).copied().flatten(),
                    mount: volume.mount,
                    label: volume.label,
                    free_bytes: volume.free_bytes,
                    total_bytes: volume.total_bytes,
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
        let sample = state.sample(Instant::now(), true).expect("disk sample");
        assert!(sample.read_bytes_per_sec >= 0.0);
        assert!(sample.write_bytes_per_sec >= 0.0);
        assert!((0.0..=100.0).contains(&sample.busy_percent));
        assert!(!sample.volumes.is_empty());
        assert!(sample.volumes.iter().all(|v| v.total_bytes > 0));
        for volume in &sample.volumes {
            if let Some(temperature) = volume.temperature_c {
                assert!(
                    (-20.0..=125.0).contains(&temperature),
                    "implausible drive temperature: {temperature} °C"
                );
            }
        }
    }
}
