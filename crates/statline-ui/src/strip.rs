//! The taskbar strip model: what to show, in what order.
//!
//! Pure data so it can be tested without a window, D2D or a live system.

use statline_core::model::Snapshot;
use statline_core::units::{UnitSystem, format_percent, format_rate_short};

/// Text used to reserve a stable width, so a changing number cannot make the
/// strip jump around. Network is the worst offender: "1.1 KiB/s" and
/// "12.4 MiB/s" must occupy the same slot.
const RATE_SAMPLE_BINARY: &str = "\u{2193} 99.9 MiB/s";
const RATE_SAMPLE_DECIMAL: &str = "\u{2193} 99.9 MB/s";

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
        // Same thresholds as the flyout: CPU and memory saturate around 95 %,
        // while a GPU or disk can sit at 97 % and still be healthy, so those two
        // only turn red at 98 %. Diverging here made the strip and the panel
        // disagree about the same number.
        let (warn, critical, width_sample) = match label {
            "CPU" | "MEM" => (85.0, 95.0, "100%"),
            "GPU" | "DISK" => (90.0, 98.0, "100%"),
            _ => (0.0, 0.0, ""),
        };
        Self {
            label,
            value,
            extra: None,
            percent,
            warn,
            critical,
            width_sample,
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
    segments_ordered(snapshot, units, visible, None)
}

/// Build the strip contents in an explicit left-to-right order.
///
/// `order` lists module labels as the settings page names them ("CPU",
/// "Memory", …). Slots not mentioned keep their default relative order after
/// the listed ones, so a partial list still renders something sane.
pub fn segments_ordered(
    snapshot: &Snapshot,
    units: UnitSystem,
    visible: VisibleModules,
    order: Option<&[&str]>,
) -> Vec<Segment> {
    let mut segments = Vec::new();
    let sequence: Vec<&str> = match order {
        Some(list) if !list.is_empty() => list.to_vec(),
        _ => DEFAULT_ORDER.to_vec(),
    };

    for name in sequence {
        match name {
            "Network" if visible.network => push_network(&mut segments, snapshot, units),
            "CPU" if visible.cpu => push_cpu(&mut segments, snapshot),
            "Memory" if visible.memory => push_memory(&mut segments, snapshot),
            "GPU" if visible.gpu => push_gpu(&mut segments, snapshot),
            "Disk" if visible.disk => push_disk(&mut segments, snapshot),
            _ => {}
        }
    }

    segments
}

/// Order used when settings say nothing: network first, for the reason above.
pub const DEFAULT_ORDER: [&str; 5] = ["Network", "CPU", "Memory", "GPU", "Disk"];

fn push_network(segments: &mut Vec<Segment>, snapshot: &Snapshot, units: UnitSystem) {
    let Some(network) = snapshot.network.as_ref() else {
        return;
    };
    // Two stacked lines, no label: the arrows say which direction is which, and
    // dropping the label keeps the segment narrow.
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

fn push_cpu(segments: &mut Vec<Segment>, snapshot: &Snapshot) {
    if let Some(cpu) = snapshot.cpu.as_ref() {
        segments.push(Segment::new(
            "CPU",
            format_percent(cpu.total_percent),
            Some(cpu.total_percent),
        ));
    }
}

fn push_memory(segments: &mut Vec<Segment>, snapshot: &Snapshot) {
    if let Some(memory) = snapshot.memory.as_ref() {
        let percent = memory.percent();
        segments.push(Segment::new("MEM", format_percent(percent), Some(percent)));
    }
}

fn push_gpu(segments: &mut Vec<Segment>, snapshot: &Snapshot) {
    if let Some(gpu) = snapshot.gpu.as_ref() {
        segments.push(Segment::new(
            "GPU",
            format_percent(gpu.utilization_percent),
            Some(gpu.utilization_percent),
        ));
    }
}

fn push_disk(segments: &mut Vec<Segment>, snapshot: &Snapshot) {
    if let Some(disk) = snapshot.disk.as_ref() {
        segments.push(Segment::new(
            "DISK",
            format_percent(disk.busy_percent),
            Some(disk.busy_percent),
        ));
    }
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
        assert_eq!(segments[0].value, "\u{2193} 1.18 MiB/s");
        assert_eq!(segments[0].extra.as_deref(), Some("\u{2191} 82.0 KiB/s"));
        assert_eq!(segments[0].width_sample, RATE_SAMPLE_BINARY);
    }

    #[test]
    fn explicit_order_moves_segments_without_losing_any() {
        let snapshot = Snapshot {
            cpu: Some(CpuSample {
                total_percent: 10.0,
                ..Default::default()
            }),
            memory: Some(MemorySample {
                used_bytes: 8,
                total_bytes: 16,
                ..Default::default()
            }),
            gpu: Some(statline_core::model::GpuSample::default()),
            disk: Some(statline_core::model::DiskSample::default()),
            network: Some(NetworkSample {
                rx_bytes_per_sec: 1024.0,
                tx_bytes_per_sec: 512.0,
                ..Default::default()
            }),
            ..Default::default()
        };

        let default = segments(&snapshot);
        assert_eq!(default[0].label, "", "network leads by default");
        assert_eq!(default.len(), 5);

        // Network last: every percentage keeps its identity, only the order moves.
        let reordered = segments_ordered(
            &snapshot,
            UnitSystem::Binary,
            VisibleModules::default(),
            Some(&["CPU", "Memory", "GPU", "Disk", "Network"]),
        );
        let labels: Vec<&str> = reordered.iter().map(|segment| segment.label).collect();
        assert_eq!(labels, vec!["CPU", "MEM", "GPU", "DISK", ""]);
        assert_eq!(reordered.len(), default.len());
    }

    #[test]
    fn order_skips_hidden_modules() {
        let snapshot = Snapshot {
            cpu: Some(CpuSample {
                total_percent: 10.0,
                ..Default::default()
            }),
            gpu: Some(statline_core::model::GpuSample::default()),
            ..Default::default()
        };
        let visible = VisibleModules {
            cpu: true,
            memory: false,
            gpu: true,
            disk: false,
            network: false,
        };
        let segments = segments_ordered(
            &snapshot,
            UnitSystem::Binary,
            visible,
            Some(&["Disk", "GPU", "Memory", "Network", "CPU"]),
        );
        let labels: Vec<&str> = segments.iter().map(|segment| segment.label).collect();
        assert_eq!(labels, vec!["GPU", "CPU"]);
    }

    #[test]
    fn thresholds_match_the_flyout_per_module() {
        let snapshot = Snapshot {
            cpu: Some(CpuSample {
                total_percent: 50.0,
                ..Default::default()
            }),
            memory: Some(MemorySample {
                used_bytes: 8,
                total_bytes: 16,
                ..Default::default()
            }),
            gpu: Some(statline_core::model::GpuSample::default()),
            disk: Some(statline_core::model::DiskSample::default()),
            ..Default::default()
        };
        let segments = segments(&snapshot);
        let find = |label: &str| {
            segments
                .iter()
                .find(|segment| segment.label == label)
                .unwrap_or_else(|| panic!("{label} segment"))
        };
        for label in ["CPU", "MEM"] {
            assert_eq!(
                (find(label).warn, find(label).critical),
                (85.0, 95.0),
                "{label}"
            );
        }
        for label in ["GPU", "DISK"] {
            assert_eq!(
                (find(label).warn, find(label).critical),
                (90.0, 98.0),
                "{label}"
            );
        }
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
        assert_eq!(segments[0].value, "\u{2193} 1.24 MB/s");
        assert_eq!(segments[0].extra.as_deref(), Some("\u{2191} 84.0 KB/s"));
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
