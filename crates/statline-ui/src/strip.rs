//! The taskbar strip model: what to show, in what order.
//!
//! Pure data so it can be tested without a window, D2D or a live system.

use statline_core::model::Snapshot;
use statline_core::units::{UnitSystem, format_percent, format_rate_short};

/// Text used to reserve a stable width, so a changing number cannot make the
/// strip jump around. Network is the worst offender: "1.1 KiB/s" and
/// "12.4 MiB/s" must occupy the same slot.
const RATE_SAMPLE_BINARY: &str = "\u{2193} 88.8 MiB/s";
const RATE_SAMPLE_DECIMAL: &str = "\u{2193} 88.8 MB/s";

/// One readout in the strip.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// Short label ("CPU"); empty for the stacked network segment.
    pub label: &'static str,
    /// Primary line ("23%").
    pub value: String,
    /// Optional second line, drawn under the first, in place of the label.
    pub extra: Option<String>,
    /// Meter fill, 0.0..=100.0. `None` hides the meter.
    pub percent: Option<f32>,
    /// Warning threshold for the value/meter colour (0 disables it).
    pub warn: f32,
    /// Critical threshold for the value/meter colour (0 disables it).
    pub critical: f32,
    /// Widest text this segment must accommodate, for stable sizing.
    pub width_sample: &'static str,
}

impl Segment {
    fn new(label: &'static str, value: String, percent: Option<f32>) -> Self {
        let (warn, critical) = match label {
            "CPU" | "MEM" => (85.0, 95.0),
            "GPU" | "DISK" => (90.0, 98.0),
            _ => (0.0, 0.0),
        };
        Self {
            label,
            value,
            extra: None,
            percent,
            warn,
            critical,
            width_sample: "",
        }
    }
}

/// Modules the strip should show. The flyout and companion remain complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleModules {
    pub cpu: bool,
    pub memory: bool,
    pub gpu: bool,
    pub disk: bool,
    pub network: bool,
}

impl Default for VisibleModules {
    fn default() -> Self {
        Self {
            cpu: true,
            memory: true,
            gpu: true,
            disk: true,
            network: true,
        }
    }
}

/// Build the strip contents from a snapshot, skipping unavailable modules.
///
/// Binary units are the default; the settings window can switch to decimal.
pub fn segments(snapshot: &Snapshot) -> Vec<Segment> {
    segments_with(snapshot, UnitSystem::Binary, VisibleModules::default())
}

/// Build the strip contents with explicit byte units and module visibility.
pub fn segments_with(
    snapshot: &Snapshot,
    units: UnitSystem,
    visible: VisibleModules,
) -> Vec<Segment> {
    let mut segments = Vec::new();

    // Network goes first on purpose: the strip is right-anchored to the tray,
    // so the only variable-width segment changes the strip's left edge. Every
    // other segment keeps its screen position instead of shifting.
    if visible.network {
        if let Some(network) = snapshot.network.as_ref() {
            // Two stacked lines, no label: the arrows say which direction is
            // which, and dropping the label keeps the segment narrow.
            segments.push(Segment {
                label: "",
                value: format!(
                    "\u{2193} {}",
                    format_rate_short(network.rx_bytes_per_sec, units)
                ),
                extra: Some(format!(
                    "\u{2191} {}",
                    format_rate_short(network.tx_bytes_per_sec, units)
                )),
                percent: None,
                warn: 0.0,
                critical: 0.0,
                width_sample: match units {
                    UnitSystem::Binary => RATE_SAMPLE_BINARY,
                    UnitSystem::Decimal => RATE_SAMPLE_DECIMAL,
                },
            });
        }
    }

    if visible.cpu {
        if let Some(cpu) = snapshot.cpu.as_ref() {
            segments.push(Segment::new(
                "CPU",
                format_percent(cpu.total_percent),
                Some(cpu.total_percent),
            ));
        }
    }

    if visible.memory {
        if let Some(memory) = snapshot.memory.as_ref() {
            let percent = memory.percent();
            segments.push(Segment::new("MEM", format_percent(percent), Some(percent)));
        }
    }

    if visible.gpu {
        if let Some(gpu) = snapshot.gpu.as_ref() {
            segments.push(Segment::new(
                "GPU",
                format_percent(gpu.utilization_percent),
                Some(gpu.utilization_percent),
            ));
        }
    }

    if visible.disk {
        if let Some(disk) = snapshot.disk.as_ref() {
            segments.push(Segment::new(
                "DISK",
                format_percent(disk.busy_percent),
                Some(disk.busy_percent),
            ));
        }
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
        assert_eq!(labels, vec!["", "CPU", "MEM"]);
        assert_eq!(segments[1].value, "23%");
        assert_eq!(segments[2].value, "50%");
        assert_eq!(segments[0].percent, None);
    }

    #[test]
    fn network_is_two_stacked_lines() {
        let snapshot = Snapshot {
            network: Some(NetworkSample {
                rx_bytes_per_sec: 1_240_000.0,
                tx_bytes_per_sec: 84_000.0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let segments = segments(&snapshot);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].label, "");
        assert_eq!(segments[0].value, "\u{2193} 1.2 MiB/s");
        assert_eq!(segments[0].extra.as_deref(), Some("\u{2191} 82 KiB/s"));
        assert_eq!(segments[0].width_sample, RATE_SAMPLE_BINARY);
    }

    #[test]
    fn hidden_modules_are_omitted() {
        let snapshot = Snapshot {
            cpu: Some(CpuSample {
                total_percent: 23.0,
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
        let visible = VisibleModules {
            cpu: false,
            memory: true,
            gpu: true,
            disk: true,
            network: false,
        };
        let segments = segments_with(&snapshot, UnitSystem::Binary, visible);
        let labels: Vec<&str> = segments.iter().map(|segment| segment.label).collect();
        assert_eq!(labels, vec!["MEM"]);
    }

    #[test]
    fn decimal_units_change_the_network_labels() {
        let snapshot = Snapshot {
            network: Some(NetworkSample {
                rx_bytes_per_sec: 1_240_000.0,
                tx_bytes_per_sec: 84_000.0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let segments = segments_with(&snapshot, UnitSystem::Decimal, VisibleModules::default());
        assert_eq!(segments[0].value, "\u{2193} 1.2 MB/s");
        assert_eq!(segments[0].extra.as_deref(), Some("\u{2191} 84 KB/s"));
        assert_eq!(segments[0].width_sample, RATE_SAMPLE_DECIMAL);
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
