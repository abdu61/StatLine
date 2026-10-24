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

#[cfg(test)]
mod tests {
    use super::*;

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
