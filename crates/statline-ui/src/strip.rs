//! The taskbar strip model: what to show, in what order.
//!
//! Pure data so it can be tested without a window, D2D or a live system.

use statline_core::model::Snapshot;
use statline_core::units::{UnitSystem, format_percent, format_rate_compact};

/// One readout in the strip.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// Short label ("CPU").
    pub label: &'static str,
    /// Formatted value ("23%").
    pub value: String,
    /// Meter fill, 0.0..=100.0. `None` hides the meter (e.g. network).
    pub percent: Option<f32>,
}

/// Build the strip contents from a snapshot, skipping unavailable modules.
pub fn segments(snapshot: &Snapshot) -> Vec<Segment> {
    let mut segments = Vec::new();

    if let Some(cpu) = snapshot.cpu.as_ref() {
        segments.push(Segment {
            label: "CPU",
            value: format_percent(cpu.total_percent),
            percent: Some(cpu.total_percent),
        });
    }

    if let Some(memory) = snapshot.memory.as_ref() {
        let percent = memory.percent();
        segments.push(Segment {
            label: "MEM",
            value: format_percent(percent),
            percent: Some(percent),
        });
    }

    if let Some(gpu) = snapshot.gpu.as_ref() {
        segments.push(Segment {
            label: "GPU",
            value: format_percent(gpu.utilization_percent),
            percent: Some(gpu.utilization_percent),
        });
    }

    if let Some(disk) = snapshot.disk.as_ref() {
        segments.push(Segment {
            label: "DISK",
            value: format_percent(disk.busy_percent),
            percent: Some(disk.busy_percent),
        });
    }

    if let Some(network) = snapshot.network.as_ref() {
        segments.push(Segment {
            label: "NET",
            value: format!(
                "\u{2193} {} \u{2191} {}",
                format_rate_compact(network.rx_bytes_per_sec, UnitSystem::Binary),
                format_rate_compact(network.tx_bytes_per_sec, UnitSystem::Binary)
            ),
            percent: None,
        });
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use statline_core::model::{CpuSample, MemorySample, NetworkSample};

    #[test]
    fn builds_segments_for_present_modules_only() {
        let snapshot = Snapshot {
            cpu: Some(CpuSample {
                total_percent: 23.4,
                ..Default::default()
            }),
            memory: Some(MemorySample {
                used_bytes: 8,
                total_bytes: 16,
                ..Default::default()
            }),
            network: Some(NetworkSample {
                rx_bytes_per_sec: 1024.0,
                tx_bytes_per_sec: 512.0,
                ..Default::default()
            }),
            ..Default::default()
        };

        let segments = segments(&snapshot);
        let labels: Vec<&str> = segments.iter().map(|s| s.label).collect();
        assert_eq!(labels, vec!["CPU", "MEM", "NET"]);
        assert_eq!(segments[0].value, "23%");
        assert_eq!(segments[1].value, "50%");
        assert_eq!(segments[2].percent, None);
    }

    #[test]
    fn empty_snapshot_produces_no_segments() {
        assert!(segments(&Snapshot::default()).is_empty());
    }

    #[test]
    fn gpu_and_disk_appear_when_present() {
        let snapshot = Snapshot {
            gpu: Some(statline_core::model::GpuSample {
                utilization_percent: 8.0,
                ..Default::default()
            }),
            disk: Some(statline_core::model::DiskSample {
                busy_percent: 4.0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let segments = segments(&snapshot);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].label, "GPU");
        assert_eq!(segments[1].label, "DISK");
    }
}
