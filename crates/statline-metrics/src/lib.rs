//! The sampling engine: turns raw Win32 readings into a `Snapshot`.

use statline_core::model::{CpuSample, MemorySample, Snapshot, SystemSample};
use statline_win::metrics::{self, CpuTimes};
use std::time::SystemTime;

pub mod cpu;

/// Holds the previous counter readings needed to compute rates.
///
/// One `Sampler` per process: the delta state is not shareable, which keeps the
/// math honest (two consumers cannot accidentally interleave readings).
#[derive(Debug, Default)]
pub struct Sampler {
    previous_total: Option<CpuTimes>,
    previous_cores: Vec<CpuTimes>,
}

impl Sampler {
    pub fn new() -> Self {
        Self {
            previous_total: None,
            previous_cores: Vec::new(),
        }
    }

    /// Take one reading of everything M1 knows about.
    ///
    /// This is a direct set of cheap syscalls on the calling thread; the
    /// sampling thread wrapper arrives with the timer in a later slice.
    pub fn sample(&mut self) -> Snapshot {
        Snapshot {
            taken_at: Some(SystemTime::now()),
            cpu: self.sample_cpu(),
            memory: sample_memory(),
            system: sample_system(),
            ..Snapshot::default()
        }
    }

    fn sample_cpu(&mut self) -> Option<CpuSample> {
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

        self.previous_total = Some(total);
        self.previous_cores = cores;

        Some(CpuSample {
            total_percent,
            per_core_percent,
            mhz: metrics::current_mhz(),
        })
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

        let system = snapshot.system.expect("system sample");
        assert!(system.uptime_secs > 0);
        assert!(system.processes > 0);
    }
}
