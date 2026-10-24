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
    /// Widest text this segment must accommodate, for stable sizing.
    pub width_sample: &'static str,
}

impl Segment {
    fn new(label: &'static str, value: String, percent: Option<f32>) -> Self {
        Self {
            label,
            value,
            extra: None,
            percent,
            width_sample: "",
        }
    }
}

/// Build the strip contents from a snapshot, skipping unavailable modules.
///
/// Binary units are the default; the settings window can switch to decimal.
pub fn segments(snapshot: &Snapshot) -> Vec<Segment> {
    segments_with(snapshot, UnitSystem::Binary)
}

/// Build the strip contents with explicit byte units.
pub fn segments_with(snapshot: &Snapshot, units: UnitSystem) -> Vec<Segment> {
    let mut segments = Vec::new();

    if let Some(cpu) = snapshot.cpu.as_ref() {
        segments.push(Segment::new(
            "CPU",
            format_percent(cpu.total_percent),
            Some(cpu.total_percent),
        ));
    }

    if let Some(memory) = snapshot.memory.as_ref() {
        let percent = memory.percent();
        segments.push(Segment::new("MEM", format_percent(percent), Some(percent)));
    }

    if let Some(gpu) = snapshot.gpu.as_ref() {
        segments.push(Segment::new(
            "GPU",
            format_percent(gpu.utilization_percent),
            Some(gpu.utilization_percent),
        ));
    }

    if let Some(disk) = snapshot.disk.as_ref() {
        segments.push(Segment::new(
            "DISK",
            format_percent(disk.busy_percent),
            Some(disk.busy_percent),
        ));
    }

    if let Some(network) = snapshot.network.as_ref() {
        // Two stacked lines, no label: the arrows say which direction is which,
        // and dropping the label keeps the segment narrow and stable.
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
            width_sample: match units {
                UnitSystem::Binary => RATE_SAMPLE_BINARY,
                UnitSystem::Decimal => RATE_SAMPLE_DECIMAL,
            },
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
        assert_eq!(labels, vec!["CPU", "MEM", ""]);
        assert_eq!(segments[0].value, "23%");
        assert_eq!(segments[1].value, "50%");
        assert_eq!(segments[2].percent, None);
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
    fn decimal_units_change_the_network_labels() {
        let snapshot = Snapshot {
            network: Some(NetworkSample {
                rx_bytes_per_sec: 1_240_000.0,
                tx_bytes_per_sec: 84_000.0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let segments = segments_with(&snapshot, UnitSystem::Decimal);
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
