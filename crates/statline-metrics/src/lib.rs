//! The sampling engine: turns raw Win32 readings into a `Snapshot`.

use statline_core::model::{
    CpuSample, MemorySample, ProcessSample, Snapshot, SystemSample, ThermalSample,
};
use statline_win::metrics::net::InterfaceRef;
use statline_win::metrics::{self, CpuTimes};
use std::time::{Duration, Instant, SystemTime};

pub mod battery;
pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod network;
pub mod process;
pub mod thermal;
pub mod thread;

use battery::BatteryState;
use disk::DiskState;
use gpu::GpuState;
use network::NetworkState;
use process::ProcessState;
use thermal::ThermalState;

/// Interface enumeration costs ~1.2 ms; the list changes rarely.
const INTERFACE_REFRESH: Duration = Duration::from_secs(60);

/// Holds the previous counter readings needed to compute rates.
///
/// One `Sampler` per process: the delta state is not shareable, which keeps the
/// math honest (two consumers cannot accidentally interleave readings).
pub struct Sampler {
    previous_total: Option<CpuTimes>,
    previous_cores: Vec<CpuTimes>,
    last_tick: Option<Instant>,
    network: NetworkState,
    interfaces: Vec<InterfaceRef>,
    interfaces_at: Option<Instant>,
    disk: DiskState,
    gpu: GpuState,
    process: ProcessState,
    thermal: ThermalState,
    battery: BatteryState,
    /// Smoothed CPU percentages for the 1/5/15 minute figures.
    average_1m: f32,
    average_5m: f32,
    average_15m: f32,
    averages_initialized: bool,
    /// Set by the UI while a surface that shows processes or sensors is open.
    details_visible: bool,
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Sampler {
    pub fn new() -> Self {
        Self {
            previous_total: None,
            previous_cores: Vec::new(),
            last_tick: None,
            network: NetworkState::new(),
            interfaces: Vec::new(),
            interfaces_at: None,
            disk: DiskState::new(),
            gpu: GpuState::new(),
            process: ProcessState::new(),
            thermal: ThermalState::new(),
            battery: BatteryState::new(),
            average_1m: 0.0,
            average_5m: 0.0,
            average_15m: 0.0,
            averages_initialized: false,
            details_visible: false,
        }
    }

    /// Enable the expensive detail providers: process table, ACPI zones, drive
    /// temperatures and battery health.
    ///
    /// The strip only needs the cheap counters. The process query allocates in
    /// the kernel and the sensor IOCTLs cost milliseconds, so they run only
    /// while the flyout or the companion is open. Disabling drops the process
    /// delta baseline so a later enable does not average over the hidden gap.
    pub fn set_detail_surfaces(&mut self, visible: bool) {
        if self.details_visible == visible {
            return;
        }
        self.details_visible = visible;
        self.process.reset();
        self.disk.set_details_visible(visible);
        self.battery.set_details_visible(visible);
        self.thermal.invalidate();
    }

    /// Take one reading of everything v1 measures.
    ///
    /// Every provider is optional: a machine without a battery, GPU counters or
    /// PDH simply produces `None` for that field.
    pub fn sample(&mut self) -> Snapshot {
        let now = Instant::now();
        let elapsed = self.last_tick.map(|previous| now - previous);
        self.last_tick = Some(now);

        Snapshot {
            taken_at: Some(SystemTime::now()),
            cpu: self.sample_cpu(elapsed),
            memory: sample_memory(),
            gpu: self.gpu.sample(now),
            disk: self.disk.sample(now, self.details_visible),
            network: Some(self.sample_network(now, elapsed)),
            battery: self.battery.sample(now, self.details_visible),
            thermal: if self.details_visible {
                self.thermal
                    .sample(now)
                    .map(|system_c| ThermalSample { system_c })
            } else {
                None
            },
            system: sample_system(),
            processes: self.sample_processes(now),
        }
    }

    fn sample_processes(&mut self, now: Instant) -> Option<ProcessSample> {
        if !self.details_visible {
            return None;
        }
        self.process.sample(now, metrics::logical_core_count())
    }

    fn sample_cpu(&mut self, elapsed: Option<Duration>) -> Option<CpuSample> {
        let total = metrics::system_times()?;
        let cores = metrics::per_core_times();

        let total_percent = self
            .previous_total
            .and_then(|previous| cpu::busy_percent(previous, total))
            .unwrap_or(0.0);

        let per_core_percent = if self.previous_cores.len() == cores.len() {
            cpu::per_core_percent(&self.previous_cores, &cores)
        } else {
            Vec::new()
        };

        let valid_delta = self.previous_total.is_some();
        let dt = elapsed
            .map(|elapsed| elapsed.as_secs_f32())
            .unwrap_or(1.0)
            .clamp(0.1, 60.0);
        if !self.averages_initialized {
            // The first snapshot has no CPU delta; seed the averages from the
            // first real reading instead of smoothing up from zero.
            if valid_delta {
                self.average_1m = total_percent;
                self.average_5m = total_percent;
                self.average_15m = total_percent;
                self.averages_initialized = true;
            }
        } else {
            self.average_1m = cpu::smooth(self.average_1m, total_percent, dt, 60.0);
            self.average_5m = cpu::smooth(self.average_5m, total_percent, dt, 300.0);
            self.average_15m = cpu::smooth(self.average_15m, total_percent, dt, 900.0);
        }

        self.previous_total = Some(total);
        self.previous_cores = cores;

        Some(CpuSample {
            total_percent,
            per_core_percent,
            mhz: metrics::current_mhz(),
            average_1m_percent: self.average_1m,
            average_5m_percent: self.average_5m,
            average_15m_percent: self.average_15m,
        })
    }

    fn sample_network(
        &mut self,
        now: Instant,
        elapsed: Option<Duration>,
    ) -> statline_core::NetworkSample {
        let stale = self
            .interfaces_at
            .map(|at| now.duration_since(at) >= INTERFACE_REFRESH)
            .unwrap_or(true);
        if stale || self.interfaces.is_empty() {
            self.interfaces = metrics::net::interfaces();
            self.interfaces_at = Some(now);
        }

        let counters = self
            .interfaces
            .iter()
            .filter_map(|interface| {
                metrics::net::counters(interface).map(|(rx, tx)| (interface.name.clone(), rx, tx))
            })
            .collect();

        self.network.update(counters, elapsed)
    }
}

fn sample_memory() -> Option<MemorySample> {
    let status = metrics::memory_status()?;
    let counters = metrics::performance_counters()?;

    Some(MemorySample {
        used_bytes: status.total_bytes.saturating_sub(status.available_bytes),
        total_bytes: status.total_bytes,
        commit_bytes: counters.commit_bytes,
        commit_limit_bytes: counters.commit_limit_bytes,
        cached_bytes: counters.cached_bytes,
    })
}

fn sample_system() -> Option<SystemSample> {
    let counters = metrics::performance_counters()?;
    Some(SystemSample {
        uptime_secs: metrics::uptime_secs(),
        processes: counters.processes,
        threads: counters.threads,
        handles: counters.handles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sample_has_no_cpu_percentage() {
        let mut sampler = Sampler::new();
        let snapshot = sampler.sample();
        let cpu = snapshot.cpu.expect("cpu sample");
        assert_eq!(cpu.total_percent, 0.0);
        assert!(cpu.per_core_percent.is_empty());
    }

    #[test]
    fn second_sample_reports_plausible_values() {
        let mut sampler = Sampler::new();
        let _ = sampler.sample();
        std::thread::sleep(std::time::Duration::from_millis(60));
        let snapshot = sampler.sample();

        let cpu = snapshot.cpu.expect("cpu sample");
        assert!((0.0..=100.0).contains(&cpu.total_percent));
        assert!(!cpu.per_core_percent.is_empty());
        assert!(
            cpu.per_core_percent
                .iter()
                .all(|p| (0.0..=100.0).contains(p))
        );

        let memory = snapshot.memory.expect("memory sample");
        assert!(memory.total_bytes > 0);
        assert!(memory.used_bytes <= memory.total_bytes);

        let network = snapshot.network.expect("network sample");
        assert!(network.rx_bytes_per_sec >= 0.0);

        if let Some(disk) = snapshot.disk {
            assert!(!disk.volumes.is_empty());
        }

        if let Some(gpu) = snapshot.gpu {
            assert!((0.0..=100.0).contains(&gpu.utilization_percent));
        }

        if let Some(battery) = snapshot.battery {
            assert!((0.0..=100.0).contains(&battery.percent));
        }

        let system = snapshot.system.expect("system sample");
        assert!(system.uptime_secs > 0);
        assert!(system.processes > 0);
    }
}
