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
    pub processes: Option<ProcessSample>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuSample {
    /// Busy percentage across all cores, 0..=100.
    pub total_percent: f32,
    /// Busy percentage per logical core, 0..=100. Empty until two samples exist.
    pub per_core_percent: Vec<f32>,
    /// Current clock per logical core in MHz, in the same order; empty if the
    /// power subsystem does not report per-core clocks.
    pub per_core_mhz: Vec<u32>,
    /// Current average core clock, if the platform reports it.
    pub mhz: Option<u32>,
    /// Processor marketing name, e.g. "AMD Ryzen 7 7840U …".
    pub name: Option<String>,
    /// Nominal base clock in MHz from the registry.
    pub base_mhz: Option<u32>,
    /// Exponentially smoothed busy percentage over the last minute.
    pub average_1m_percent: f32,
    /// Exponentially smoothed busy percentage over the last five minutes.
    pub average_5m_percent: f32,
    /// Exponentially smoothed busy percentage over the last fifteen minutes.
    pub average_15m_percent: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemorySample {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub commit_bytes: u64,
    pub commit_limit_bytes: u64,
    pub cached_bytes: u64,
    /// Page-file (swap) usage in bytes; both zero when no page file exists.
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
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
    /// Adapter name from the vendor driver, when available.
    pub name: Option<String>,
    pub temperature_c: Option<f32>,
    pub fan_percent: Option<f32>,
    /// Fan speed when the firmware only reports RPM.
    pub fan_rpm: Option<u32>,
    pub power_w: Option<f32>,
    /// Current graphics clock in MHz.
    pub core_mhz: Option<u32>,
    /// Current memory clock in MHz.
    pub memory_mhz: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiskSample {
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    pub busy_percent: f32,
    pub volumes: Vec<VolumeSample>,
    /// Per-physical-disk activity, when PDH exposes named instances.
    pub disks: Vec<DiskActivitySample>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiskActivitySample {
    /// PDH instance name, e.g. `0 C:`.
    pub name: String,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    pub busy_percent: f32,
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
    /// Connected WLAN details, when the machine is on Wi-Fi.
    pub wifi: Option<WifiSample>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WifiSample {
    pub ssid: String,
    pub signal_percent: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AdapterSample {
    pub name: String,
    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
    /// Negotiated link speed in bits per second; 0 when unknown.
    pub link_speed_bps: u64,
    /// IEEE 802.11 interface.
    pub wireless: bool,
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
    /// Manufacturer cycle count, when the battery reports it.
    pub cycle_count: Option<u32>,
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
    /// Windows edition/build, e.g. "Windows 11 Pro 24H2 (build 26100.2033)".
    pub os: Option<String>,
    /// Computer name.
    pub computer: Option<String>,
    /// Installed memory modules, in firmware slot order.
    pub memory_modules: Vec<MemoryModuleInfo>,
}

/// One populated memory slot, as reported by SMBIOS.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryModuleInfo {
    /// Slot label from the firmware, e.g. `DIMM 0`.
    pub slot: String,
    /// Bank locator, used when two slots share a device locator.
    pub bank: String,
    pub size_bytes: u64,
    /// Configured speed in MT/s.
    pub speed_mts: u32,
    /// Generation name such as `DDR4`; empty when the firmware omits it.
    pub kind: String,
}

/// The per-app breakdown.
///
/// Only populated while a surface that shows processes is visible, so the
/// process-table query never runs in the background.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessSample {
    /// Processes the kernel reported, before the top-N cut.
    pub total: u32,
    /// Highest-CPU processes, descending.
    pub top: Vec<ProcessEntry>,
}

/// One row of the process table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessEntry {
    pub pid: u32,
    /// Executable name, e.g. `chrome.exe`.
    pub name: String,
    /// Live threads across grouped instances.
    pub threads: u32,
    /// How many processes share this executable name.
    pub instances: u32,
    /// Share of total CPU capacity (sum of all cores), 0..=100.
    ///
    /// This is the Task Manager scale: one saturated core on an eight-core
    /// machine reads 12.5 %, and the column sums to the CPU total. `None`
    /// until a second sample provides a delta, so the UI can show "—" rather
    /// than claim a busy process is idle.
    pub cpu_percent: Option<f32>,
    pub working_set_bytes: u64,
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
