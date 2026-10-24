//! The immutable snapshot the sampler publishes and every view reads.
//!
//! `Option::None` means "this provider has no reading" — the UI hides the row.
//! It never means zero, and no view may substitute a default.

use std::time::SystemTime;

/// One complete sample of everything currently being measured.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub taken_at: Option<SystemTime>,
    pub cpu: Option<CpuSample>,
    pub memory: Option<MemorySample>,
    pub gpu: Option<GpuSample>,
    pub disk: Option<DiskSample>,
    pub network: Option<NetworkSample>,
    pub battery: Option<BatterySample>,
    pub thermal: Option<ThermalSample>,
    pub system: Option<SystemSample>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuSample {
    /// Busy percentage across all cores, 0..=100.
    pub total_percent: f32,
    /// Busy percentage per logical core, 0..=100. Empty until two samples exist.
    pub per_core_percent: Vec<f32>,
    /// Current average core clock, if the platform reports it.
    pub mhz: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemorySample {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub commit_bytes: u64,
    pub commit_limit_bytes: u64,
    pub cached_bytes: u64,
}

impl MemorySample {
    /// `used / total` as a percentage, clamped to 0..=100.
    pub fn percent(&self) -> f32 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.used_bytes as f64 / self.total_bytes as f64 * 100.0).clamp(0.0, 100.0) as f32
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GpuSample {
    pub utilization_percent: f32,
    pub vram_used_bytes: u64,
    pub vram_total_bytes: u64,
    pub temperature_c: Option<f32>,
    pub fan_percent: Option<f32>,
    pub power_w: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiskSample {
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    pub busy_percent: f32,
    pub volumes: Vec<VolumeSample>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VolumeSample {
    pub mount: String,
    pub label: String,
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub temperature_c: Option<f32>,
}

impl VolumeSample {
    /// Used space in bytes.
    pub fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.free_bytes)
    }

    pub fn used_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.used_bytes() as f64 / self.total_bytes as f64 * 100.0).clamp(0.0, 100.0) as f32
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetworkSample {
    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
    pub adapters: Vec<AdapterSample>,
    /// Bytes received since the app started.
    pub session_rx_bytes: u64,
    /// Bytes sent since the app started.
    pub session_tx_bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AdapterSample {
    pub name: String,
    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BatterySample {
    pub percent: f32,
    pub on_ac: bool,
    /// Seconds of runtime remaining; `None` while charging or when unknown.
    pub seconds_left: Option<u32>,
    /// Instantaneous charge (+) or discharge (−) rate in milliwatts.
    pub rate_mw: Option<i32>,
    pub designed_mwh: Option<u32>,
    pub full_mwh: Option<u32>,
    pub temperature_c: Option<f32>,
}

impl BatterySample {
    /// Full-charge capacity as a percentage of design capacity.
    pub fn health_percent(&self) -> Option<f32> {
        match (self.designed_mwh, self.full_mwh) {
            (Some(designed), Some(full)) if designed > 0 => {
                Some((full as f64 / designed as f64 * 100.0).clamp(0.0, 200.0) as f32)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThermalSample {
    /// ACPI thermal-zone reading. Labelled "System" in the UI, never "CPU".
    pub system_c: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SystemSample {
    pub uptime_secs: u64,
    pub processes: u32,
    pub threads: u32,
    pub handles: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_percent_handles_zero_total() {
        let sample = MemorySample::default();
        assert_eq!(sample.percent(), 0.0);
    }

    #[test]
    fn memory_percent_is_clamped() {
        let sample = MemorySample {
            used_bytes: 20,
            total_bytes: 10,
            ..Default::default()
        };
        assert_eq!(sample.percent(), 100.0);
    }

    #[test]
    fn volume_used_accounts_for_free_space() {
        let volume = VolumeSample {
            free_bytes: 400,
            total_bytes: 1000,
            ..Default::default()
        };
        assert_eq!(volume.used_bytes(), 600);
        assert!((volume.used_percent() - 60.0).abs() < f32::EPSILON);
    }

    #[test]
    fn battery_health_needs_both_capacities() {
        let mut battery = BatterySample::default();
        assert_eq!(battery.health_percent(), None);
        battery.designed_mwh = Some(50_000);
        battery.full_mwh = Some(45_000);
        assert_eq!(battery.health_percent(), Some(90.0));
    }
}
