//! GPU utilization (PDH) and video memory usage.

use crate::metrics::pdh::{PdhCounter, PdhQuery};

/// GPU load and memory, when the machine exposes them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GpuCounters2 {
    pub utilization_percent: f32,
    pub vram_used_bytes: u64,
    pub vram_total_bytes: u64,
}

/// Holds the PDH wildcard counters for GPU engines and process memory.
///
/// One query with three wildcard counters: `PdhCollectQueryData` walks every
/// instance the query contains, so keeping them together avoids paying that
/// cost more than once.
pub struct GpuCounters {
    query: PdhQuery,
    engines: PdhCounter,
    local: PdhCounter,
    shared: PdhCounter,
}

impl GpuCounters {
    pub fn open() -> Option<Self> {
        let query = PdhQuery::open()?;
        let engines = query.add_english("\\GPU Engine(*)\\Utilization Percentage")?;
        // Per-process memory. This is the source Task Manager derives
        // "Dedicated GPU memory" from, and the only one that reports useful
        // numbers on integrated GPUs.
        let local = query.add_english("\\GPU Process Memory(*)\\Local Usage")?;
        let shared = query.add_english("\\GPU Process Memory(*)\\Non Local Usage")?;
        let _ = query.collect();
        Some(Self {
            query,
            engines,
            local,
            shared,
        })
    }

    /// Sum of every engine's utilization, clamped to 100 %, plus memory usage.
    ///
    /// Engines are per-process and per-engine-type, so the sum over all
    /// instances approximates the whole-GPU busy time Task Manager shows.
    pub fn sample(&self) -> Option<GpuCounters2> {
        if !self.query.collect() {
            return None;
        }

        let total: f64 = self
            .engines
            .values()
            .unwrap_or_default()
            .into_iter()
            .filter(|value| *value > 0.0)
            .sum();

        let used = sum_positive(&self.local) + sum_positive(&self.shared);
        let (_dxgi_used, budget) = crate::metrics::dxgi::video_memory().unwrap_or((0, 0));

        Some(GpuCounters2 {
            utilization_percent: total.clamp(0.0, 100.0) as f32,
            vram_used_bytes: used,
            vram_total_bytes: budget,
        })
    }
}

fn sum_positive(counter: &PdhCounter) -> u64 {
    counter
        .values()
        .unwrap_or_default()
        .into_iter()
        .filter(|value| *value > 0.0)
        .map(|value| value as u64)
        .sum()
}

/// Extract the PID from a `\GPU Engine(...)` instance name.
///
/// The instance looks like
/// `pid_1234_luid_0x00000000_0x0000A1B2_phys_0_eng_0_engtype_3D`. Only the
/// `pid_` field is needed, and the rest of the name is irrelevant.
pub fn pid_from_instance(name: &str) -> Option<u32> {
    let rest = name.split("pid_").nth(1)?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

impl GpuCounters {
    /// GPU utilization per process, in percent of total GPU capacity.
    ///
    /// Each engine instance belongs to one process, so summing an engine's
    /// values per PID gives that process's share. Uses the query already
    /// collected by [`Self::sample`], so this adds no PDH cost.
    pub fn utilization_by_process(&self) -> Vec<(u32, f32)> {
        let mut per_pid: std::collections::HashMap<u32, f32> = std::collections::HashMap::new();
        for (name, value) in self.engines.named_values().unwrap_or_default() {
            if value <= 0.0 {
                continue;
            }
            if let Some(pid) = pid_from_instance(&name) {
                *per_pid.entry(pid).or_default() += value as f32;
            }
        }
        let mut rows: Vec<(u32, f32)> = per_pid
            .into_iter()
            .map(|(pid, percent)| (pid, percent.clamp(0.0, 100.0)))
            .collect();
        rows.sort_by(|a, b| b.1.total_cmp(&a.1));
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_names_yield_pids() {
        assert_eq!(
            pid_from_instance("pid_4321_luid_0x00000000_0x0000A1B2_phys_0_eng_0_engtype_3D"),
            Some(4321)
        );
        assert_eq!(pid_from_instance("gpuengine_weird"), None);
        assert_eq!(pid_from_instance("pid__x"), None);
    }

    #[test]
    fn opens_only_when_gpu_counters_exist() {
        // Absent GPU performance counters (VMs, some drivers) must not panic.
        if let Some(counters) = GpuCounters::open() {
            std::thread::sleep(std::time::Duration::from_millis(80));
            if let Some(sample) = counters.sample() {
                assert!((0.0..=100.0).contains(&sample.utilization_percent));
                assert!(
                    sample.vram_used_bytes <= sample.vram_total_bytes
                        || sample.vram_total_bytes == 0
                );
            }
        }
    }
}
