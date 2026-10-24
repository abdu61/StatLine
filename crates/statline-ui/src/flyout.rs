//! The flyout panel: a compact summary that opens above the strip.

use statline_core::model::{ProcessEntry, Snapshot};
use statline_core::units::{
    UnitSystem, format_bytes, format_duration, format_percent, format_percent_precise, format_rate,
    format_rate_short, format_temperature,
};
use statline_render::{Align, Color, Palette, Surface, TextEngine, TextStyle, radius, space};

/// Series kept for the sparklines, oldest first, 0..=100.
#[derive(Debug, Default, Clone)]
pub struct History {
    pub cpu: Vec<f32>,
    pub memory: Vec<f32>,
    pub gpu: Vec<f32>,
}

/// The module a strip segment belongs to, used to focus the matching card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Cpu,
    Memory,
    Gpu,
    Disk,
    Network,
}

impl Focus {
    /// Label used by the corresponding card, which is how the renderer finds it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
            Self::Gpu => "GPU",
            Self::Disk => "Disk",
            Self::Network => "Network",
        }
    }
}

/// One card in the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub label: &'static str,
    pub value: String,
    pub detail: String,
    /// Sparkline samples, 0..=100; empty hides the chart.
    pub series: Vec<f32>,
}

/// One line of the process table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessRow {
    pub name: String,
    pub cpu: String,
    pub memory: String,
}

/// Everything the panel shows.
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub header: String,
    pub rows: Vec<Row>,
    /// Busiest processes, already sorted; empty hides the section.
    pub processes: Vec<ProcessRow>,
    /// Module the panel was opened from, highlighted and used to sort processes.
    pub focus: Option<Focus>,
}

/// Process rows the flyout shows (the companion shows more).
pub const PROCESS_ROWS: usize = 5;
const PROCESS_ROW_HEIGHT: f32 = 17.0;

fn series(values: &[f32], latest: f32) -> Vec<f32> {
    let mut series = values.to_vec();
    if series.is_empty() {
        series.push(latest);
    }
    series
}

/// Build the panel contents from a snapshot plus the recorded history.
pub fn model(snapshot: &Snapshot, history: &History) -> Model {
    model_focused(snapshot, history, None)
}

/// Build the panel with one module focused.
///
/// Focusing memory sorts the process table by working set; every other module
/// keeps the sampler's CPU order.
pub fn model_focused(snapshot: &Snapshot, history: &History, focus: Option<Focus>) -> Model {
    let mut rows = Vec::new();

    let header = match snapshot.system.as_ref() {
        Some(system) => format!(
            "Up {} · {} processes",
            format_duration(system.uptime_secs),
            system.processes
        ),
        None => String::from("Statline"),
    };

    if let Some(cpu) = snapshot.cpu.as_ref() {
        let detail = match cpu.mhz {
            Some(mhz) => format!("{} cores · {} MHz", cpu.per_core_percent.len().max(1), mhz),
            None => format!("{} cores", cpu.per_core_percent.len().max(1)),
        };
        rows.push(Row {
            label: "CPU",
            value: format_percent(cpu.total_percent),
            detail,
            series: series(&history.cpu, cpu.total_percent),
        });
    }

    if let Some(memory) = snapshot.memory.as_ref() {
        rows.push(Row {
            label: "Memory",
            value: format_percent(memory.percent()),
            detail: format!(
                "{} of {} · commit {}",
                format_bytes(memory.used_bytes, UnitSystem::Binary),
                format_bytes(memory.total_bytes, UnitSystem::Binary),
                format_bytes(memory.commit_bytes, UnitSystem::Binary)
            ),
            series: series(&history.memory, memory.percent()),
        });
    }

    if let Some(gpu) = snapshot.gpu.as_ref() {
        let detail = if gpu.vram_total_bytes > 0 {
            format!(
                "VRAM {} of {}",
                format_bytes(gpu.vram_used_bytes, UnitSystem::Binary),
                format_bytes(gpu.vram_total_bytes, UnitSystem::Binary)
            )
        } else {
            String::from("VRAM unavailable")
        };
        rows.push(Row {
            label: "GPU",
            value: format_percent(gpu.utilization_percent),
            detail,
            series: series(&history.gpu, gpu.utilization_percent),
        });
    }

    if let Some(disk) = snapshot.disk.as_ref() {
        let detail = match disk.volumes.first() {
            Some(volume) => {
                let temperature = volume
                    .temperature_c
                    .map(|celsius| format!(" \u{00b7} {}", format_temperature(celsius)))
                    .unwrap_or_default();
                format!(
                    "{} free of {} \u{00b7} read {} write {}{temperature}",
                    format_bytes(volume.free_bytes, UnitSystem::Binary),
                    format_bytes(volume.total_bytes, UnitSystem::Binary),
                    format_rate(disk.read_bytes_per_sec, UnitSystem::Binary),
                    format_rate(disk.write_bytes_per_sec, UnitSystem::Binary)
                )
            }
            None => format!(
                "read {} write {}",
                format_rate(disk.read_bytes_per_sec, UnitSystem::Binary),
                format_rate(disk.write_bytes_per_sec, UnitSystem::Binary)
            ),
        };
        rows.push(Row {
            label: "Disk",
            value: format_percent(disk.busy_percent),
            detail,
            series: Vec::new(),
        });
    }

    if let Some(network) = snapshot.network.as_ref() {
        rows.push(Row {
            label: "Network",
            value: format!(
                "\u{2193} {}  \u{2191} {}",
                format_rate_short(network.rx_bytes_per_sec, UnitSystem::Decimal),
                format_rate_short(network.tx_bytes_per_sec, UnitSystem::Decimal)
            ),
            detail: match network.adapters.first() {
                Some(adapter) => adapter.name.clone(),
                None => String::from("no active adapter"),
            },
            series: Vec::new(),
        });
    }

    if let Some(battery) = snapshot.battery.as_ref() {
        let mut detail = match battery.seconds_left {
            Some(seconds) => format!(
                "{} remaining{}",
                format_duration(seconds as u64),
                if battery.on_ac { " (charging)" } else { "" }
            ),
            None => String::from(if battery.on_ac { "on AC" } else { "on battery" }),
        };
        if let Some(health) = battery.health_percent() {
            detail.push_str(&format!(" \u{00b7} {} health", format_percent(health)));
        }
        if let Some(celsius) = battery.temperature_c {
            detail.push_str(&format!(" \u{00b7} {}", format_temperature(celsius)));
        }
        rows.push(Row {
            label: "Battery",
            value: format_percent(battery.percent),
            detail,
            series: Vec::new(),
        });
    }

    if let Some(thermal) = snapshot.thermal.as_ref() {
        let detail = match snapshot.system.as_ref() {
            Some(system) => format!(
                "Up {} \u{00b7} {} threads \u{00b7} {} handles",
                format_duration(system.uptime_secs),
                system.threads,
                system.handles
            ),
            None => String::from("ACPI thermal zone"),
        };
        rows.push(Row {
            label: "System",
            value: format_temperature(thermal.system_c),
            detail,
            series: Vec::new(),
        });
    }

    let processes = snapshot
        .processes
        .as_ref()
        .map(|sample| {
            let mut entries: Vec<&ProcessEntry> = sample.top.iter().collect();
            if focus == Some(Focus::Memory) {
                entries.sort_by(|a, b| {
                    b.working_set_bytes.cmp(&a.working_set_bytes).then_with(|| {
                        b.cpu_percent
                            .unwrap_or(0.0)
                            .total_cmp(&a.cpu_percent.unwrap_or(0.0))
                    })
                });
            }
            entries
                .into_iter()
                .map(|entry| ProcessRow {
                    name: entry.name.clone(),
                    cpu: entry
                        .cpu_percent
                        .map(format_percent_precise)
                        .unwrap_or_else(|| String::from("\u{2014}")),
                    memory: format_bytes(entry.working_set_bytes, UnitSystem::Binary),
                })
                .collect()
        })
        .unwrap_or_default();

    Model {
        header,
        rows,
        processes,
        focus,
    }
}

fn label_style(scale: f32) -> TextStyle {
    TextStyle::new(9.0 * scale, true, Align::Left)
}

fn value_style(scale: f32) -> TextStyle {
    TextStyle::new(13.0 * scale, true, Align::Right)
}

fn detail_style(scale: f32) -> TextStyle {
    TextStyle::new(9.0 * scale, false, Align::Left)
}

fn header_style(scale: f32) -> TextStyle {
    TextStyle::new(9.0 * scale, false, Align::Left)
}

fn process_style(scale: f32, semibold: bool) -> TextStyle {
    TextStyle::new(9.5 * scale, semibold, Align::Left)
}

fn process_value_style(scale: f32) -> TextStyle {
    TextStyle::new(9.5 * scale, false, Align::Right)
}

/// Panel width in physical pixels for a given scale.
pub fn width(scale: f32) -> f32 {
    300.0 * scale
}

/// Height of the process section for the rows this surface shows.
fn process_section(model: &Model, scale: f32, limit: usize) -> f32 {
    let count = model.processes.len().min(limit);
    if count == 0 {
        return 0.0;
    }
    (space::M + 1.0 + space::XS + 12.0 + space::XXS + count as f32 * PROCESS_ROW_HEIGHT) * scale
}

/// Height needed for a model.
pub fn height(model: &Model, scale: f32) -> f32 {
    let padding = space::L * scale;
    let header = 18.0 * scale;
    let row = 52.0 * scale;
    padding * 2.0
        + header
        + row * model.rows.len() as f32
        + space::S * scale
        + process_section(model, scale, PROCESS_ROWS)
}

/// Draw an area sparkline of `values` (0..=100) into `rect`.
///
/// One column per physical pixel with linear interpolation between samples, so
/// the curve reads smoothly even with only a minute of history.
fn sparkline(
    surface: &mut Surface,
    values: &[f32],
    (x, y, width, height): (f32, f32, f32, f32),
    color: Color,
    track: Color,
) {
    if width < 2.0 || height < 2.0 || values.is_empty() {
        return;
    }

    // Baseline and a very faint top rule give the chart an extent even when the
    // data is flat.
    surface.fill_rect(x, y + height - 1.0, width, 1.0, track.scaled_alpha(0.45));

    let columns = width.floor().max(1.0) as usize;
    let last = values.len().saturating_sub(1);

    for column in 0..columns {
        let position = if columns > 1 {
            column as f32 / (columns - 1) as f32 * last as f32
        } else {
            0.0
        };
        let low = position.floor() as usize;
        let high = (low + 1).min(last);
        let fraction = position - low as f32;
        let value = values[low] * (1.0 - fraction) + values[high] * fraction;

        let normalized = (value.clamp(0.0, 100.0) / 100.0).clamp(0.0, 1.0);
        let bar = (height * normalized).max(1.0);
        surface.fill_rect(x + column as f32, y + height - bar, 1.0, bar, color);
    }
}

/// Draw the panel. Returns its size in physical pixels.
pub fn render(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    model: &Model,
    scale: f32,
) -> (f32, f32) {
    let padding = space::L * scale;
    let width = width(scale);
    let height = height(model, scale);

    surface.begin(palette.surface);
    surface.fill_round_rect(
        0.0,
        0.0,
        width,
        height,
        radius::PANEL * scale,
        palette.surface,
    );

    let mut y = padding;
    let _ = text.draw(
        surface,
        &model.header,
        (padding, y, width - padding * 2.0, 16.0 * scale),
        header_style(scale),
        palette.text_muted,
    );
    y += 18.0 * scale + space::S * scale;

    for row in &model.rows {
        let row_height = 52.0 * scale;
        let focused = model.focus.is_some_and(|focus| focus.label() == row.label);

        if focused {
            surface.fill_rect(
                padding - space::XS * scale,
                y,
                2.0 * scale,
                14.0 * scale,
                palette.accent,
            );
        }

        let _ = text.draw(
            surface,
            row.label,
            (padding, y, 120.0 * scale, 14.0 * scale),
            label_style(scale),
            if focused {
                palette.accent
            } else {
                palette.text_muted
            },
        );
        let _ = text.draw(
            surface,
            &row.value,
            (
                width - padding - 140.0 * scale,
                y,
                140.0 * scale,
                16.0 * scale,
            ),
            value_style(scale),
            palette.text_primary,
        );
        let _ = text.draw(
            surface,
            &row.detail,
            (
                padding,
                y + 16.0 * scale,
                width - padding * 2.0,
                12.0 * scale,
            ),
            detail_style(scale),
            palette.text_muted,
        );

        if !row.series.is_empty() {
            sparkline(
                surface,
                &row.series,
                (
                    padding,
                    y + 29.0 * scale,
                    width - padding * 2.0,
                    16.0 * scale,
                ),
                palette.accent.scaled_alpha(0.75),
                palette.meter_track,
            );
        }

        y += row_height;
    }

    if !model.processes.is_empty() {
        y += space::M * scale;
        surface.fill_rect(
            padding,
            y,
            width - padding * 2.0,
            1.0,
            palette.surface_border,
        );
        y += 1.0 + space::XS * scale;

        let heading = match model.focus {
            Some(Focus::Memory) => "TOP PROCESSES \u{00b7} MEMORY",
            _ => "TOP PROCESSES",
        };
        let _ = text.draw(
            surface,
            heading,
            (padding, y, width - padding * 2.0, 12.0 * scale),
            TextStyle::new(8.0 * scale, true, Align::Left),
            palette.text_muted,
        );
        y += 12.0 * scale + space::XXS * scale;

        let memory_width = 78.0 * scale;
        let cpu_width = 38.0 * scale;
        let gap = space::M * scale;
        let name_width = width - padding * 2.0 - memory_width - cpu_width - gap * 2.0;

        for process in model.processes.iter().take(PROCESS_ROWS) {
            let _ = text.draw(
                surface,
                &process.name,
                (padding, y, name_width, 14.0 * scale),
                process_style(scale, false),
                palette.text_primary,
            );
            let _ = text.draw(
                surface,
                &process.cpu,
                (
                    width - padding - memory_width - gap - cpu_width,
                    y,
                    cpu_width,
                    14.0 * scale,
                ),
                process_value_style(scale),
                palette.text_primary,
            );
            let _ = text.draw(
                surface,
                &process.memory,
                (
                    width - padding - memory_width,
                    y,
                    memory_width,
                    14.0 * scale,
                ),
                process_value_style(scale),
                palette.text_muted,
            );
            y += PROCESS_ROW_HEIGHT * scale;
        }
    }

    surface.end();
    (width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use statline_core::model::{
        BatterySample, CpuSample, MemorySample, ProcessEntry, ProcessSample,
    };

    fn snapshot() -> Snapshot {
        Snapshot {
            cpu: Some(CpuSample {
                total_percent: 23.0,
                per_core_percent: vec![10.0, 20.0],
                mhz: Some(2400),
            }),
            memory: Some(MemorySample {
                used_bytes: 8,
                total_bytes: 16,
                commit_bytes: 9,
                commit_limit_bytes: 20,
                cached_bytes: 3,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn builds_rows_for_present_modules_only() {
        let model = model(&snapshot(), &History::default());
        let labels: Vec<&str> = model.rows.iter().map(|row| row.label).collect();
        assert_eq!(labels, vec!["CPU", "Memory"]);
        assert_eq!(model.rows[0].value, "23%");
        assert_eq!(model.rows[1].value, "50%");
    }

    #[test]
    fn history_feeds_the_series() {
        let history = History {
            cpu: vec![10.0, 20.0, 30.0],
            ..Default::default()
        };
        let model = model(&snapshot(), &history);
        assert_eq!(model.rows[0].series, vec![10.0, 20.0, 30.0]);
    }

    #[test]
    fn missing_history_falls_back_to_the_current_value() {
        let model = model(&snapshot(), &History::default());
        assert_eq!(model.rows[0].series, vec![23.0]);
    }

    #[test]
    fn height_grows_with_rows() {
        let model = model(&snapshot(), &History::default());
        assert!(height(&model, 1.0) > 2.0 * 46.0);
        assert!(height(&Model::default(), 1.0) > 0.0);
    }

    #[test]
    fn process_rows_are_formatted_from_the_snapshot() {
        let mut snapshot = snapshot();
        snapshot.processes = Some(ProcessSample {
            total: 240,
            top: vec![ProcessEntry {
                pid: 42,
                name: String::from("code.exe"),
                cpu_percent: Some(4.2),
                working_set_bytes: 640 * 1024 * 1024,
            }],
        });

        let model = model(&snapshot, &History::default());
        assert_eq!(model.processes.len(), 1);
        assert_eq!(model.processes[0].name, "code.exe");
        assert_eq!(model.processes[0].cpu, "4.2%");
        assert_eq!(model.processes[0].memory, "640 MiB");
    }

    #[test]
    fn process_without_a_baseline_shows_a_dash() {
        let mut snapshot = snapshot();
        snapshot.processes = Some(ProcessSample {
            total: 1,
            top: vec![ProcessEntry {
                pid: 7,
                name: String::from("new.exe"),
                cpu_percent: None,
                working_set_bytes: 1024,
            }],
        });

        let model = model(&snapshot, &History::default());
        assert_eq!(model.processes[0].cpu, "\u{2014}");
    }

    #[test]
    fn memory_focus_sorts_processes_by_working_set() {
        let mut snapshot = snapshot();
        snapshot.processes = Some(ProcessSample {
            total: 2,
            top: vec![
                ProcessEntry {
                    pid: 1,
                    name: String::from("cpu.exe"),
                    cpu_percent: Some(50.0),
                    working_set_bytes: 10,
                },
                ProcessEntry {
                    pid: 2,
                    name: String::from("memory.exe"),
                    cpu_percent: Some(1.0),
                    working_set_bytes: 1_000,
                },
            ],
        });

        let busiest = model_focused(&snapshot, &History::default(), None);
        assert_eq!(busiest.processes[0].name, "cpu.exe");
        assert_eq!(busiest.focus, None);

        let largest = model_focused(&snapshot, &History::default(), Some(Focus::Memory));
        assert_eq!(largest.processes[0].name, "memory.exe");
        assert_eq!(largest.focus, Some(Focus::Memory));
    }

    #[test]
    fn thermal_zone_adds_a_system_row() {
        let mut snapshot = snapshot();
        snapshot.thermal = Some(statline_core::model::ThermalSample { system_c: 54.0 });
        snapshot.system = Some(statline_core::model::SystemSample {
            uptime_secs: 3_600,
            processes: 240,
            threads: 3_000,
            handles: 90_000,
        });

        let model = model(&snapshot, &History::default());
        let system = model
            .rows
            .iter()
            .find(|row| row.label == "System")
            .expect("system row");
        assert_eq!(system.value, "54 °C");
        assert!(system.detail.contains("1h"), "detail: {}", system.detail);
    }

    #[test]
    fn battery_health_and_temperature_are_shown() {
        let mut snapshot = snapshot();
        snapshot.battery = Some(BatterySample {
            percent: 98.0,
            on_ac: true,
            designed_mwh: Some(50_000),
            full_mwh: Some(46_000),
            temperature_c: Some(31.0),
            ..Default::default()
        });

        let model = model(&snapshot, &History::default());
        let battery = model
            .rows
            .iter()
            .find(|row| row.label == "Battery")
            .expect("battery row");
        assert_eq!(battery.value, "98%");
        assert!(
            battery.detail.contains("92% health"),
            "detail: {}",
            battery.detail
        );
        assert!(
            battery.detail.contains("31 °C"),
            "detail: {}",
            battery.detail
        );
    }

    #[test]
    fn process_section_adds_height() {
        let plain = model(&snapshot(), &History::default());
        let mut with_processes = plain.clone();
        with_processes.processes = vec![ProcessRow::default(); 6];
        // Six rows are present but only PROCESS_ROWS are drawn.
        assert_eq!(
            height(&with_processes, 1.0) - height(&plain, 1.0),
            (space::M + 1.0 + space::XS + 12.0 + space::XXS)
                + PROCESS_ROWS as f32 * PROCESS_ROW_HEIGHT
        );
    }
}
