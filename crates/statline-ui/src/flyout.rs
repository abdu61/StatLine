//! The flyout panel: a compact summary that opens above the strip.

use statline_core::model::{ProcessEntry, Snapshot};
use statline_core::units::{
    UnitSystem, format_bytes, format_duration, format_frequency, format_percent,
    format_percent_precise, format_rate, format_rate_compact, format_rate_short,
    format_temperature,
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
    Battery,
    System,
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
            Self::Battery => "Battery",
            Self::System => "System",
        }
    }
}

/// One card in the panel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    pub label: &'static str,
    pub value: String,
    pub detail: String,
    /// Sparkline samples, 0..=100; empty hides the chart.
    pub series: Vec<f32>,
    /// Current percentage used for the warning/critical colour.
    pub current: f32,
    /// Warning threshold (0 disables it).
    pub warn: f32,
    /// Critical threshold (0 disables it).
    pub critical: f32,
}

impl Row {
    /// Colour for the value, honouring the thresholds.
    pub fn value_color(&self, palette: &Palette) -> Color {
        palette.value_color(self.current, self.warn, self.critical)
    }

    /// Colour for the chart line: the accent normally, warning/critical as the
    /// value approaches the limit.
    pub fn chart_color(&self, palette: &Palette) -> Color {
        if self.critical > 0.0 && self.current >= self.critical {
            palette.critical
        } else if self.warn > 0.0 && self.current >= self.warn {
            palette.warning
        } else {
            palette.accent
        }
    }
}

/// One line of the process table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessRow {
    pub name: String,
    pub cpu: String,
    pub memory: String,
}

/// One line of per-module detail, e.g. a volume or an adapter.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DetailRow {
    pub label: String,
    pub value: String,
}

/// One logical core in the CPU detail grid.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoreReadout {
    pub percent: f32,
    /// Current clock in MHz, when the platform reports it.
    pub mhz: Option<u32>,
}

/// Everything the panel shows.
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub header: String,
    pub rows: Vec<Row>,
    /// Focused-module lines under the first card (volumes, adapters).
    pub detail_rows: Vec<DetailRow>,
    /// Per-core readouts; drawn as a labelled grid under the CPU card.
    pub cores: Vec<CoreReadout>,
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
    model_focused(snapshot, history, None, UnitSystem::Binary)
}

/// Build the panel with one module focused and explicit byte units.
///
/// Focusing memory sorts the process table by working set; every other module
/// keeps the sampler's CPU order.
pub fn model_focused(
    snapshot: &Snapshot,
    history: &History,
    focus: Option<Focus>,
    units: UnitSystem,
) -> Model {
    let overview = focus.is_none();
    let mut rows = Vec::new();
    let mut detail_rows = Vec::new();
    let mut cores = Vec::new();

    let header = match snapshot.system.as_ref() {
        Some(system) => format!(
            "Up {} · {} processes",
            format_duration(system.uptime_secs),
            system.processes
        ),
        None => String::from("Statline"),
    };

    if let Some(cpu) = snapshot.cpu.as_ref() {
        if overview || focus == Some(Focus::Cpu) {
            let detail = match cpu.mhz {
                Some(mhz) => format!(
                    "{} cores \u{00b7} {}",
                    cpu.per_core_percent.len().max(1),
                    format_frequency(mhz)
                ),
                None => format!("{} cores", cpu.per_core_percent.len().max(1)),
            };
            rows.push(Row {
                label: "CPU",
                value: format_percent(cpu.total_percent),
                detail,
                series: series(&history.cpu, cpu.total_percent),
                current: cpu.total_percent,
                warn: 85.0,
                critical: 95.0,
            });
            if focus == Some(Focus::Cpu) {
                cores = cpu
                    .per_core_percent
                    .iter()
                    .enumerate()
                    .map(|(index, percent)| CoreReadout {
                        percent: *percent,
                        mhz: cpu.per_core_mhz.get(index).copied().filter(|mhz| *mhz > 0),
                    })
                    .collect();
                detail_rows.push(DetailRow {
                    label: String::from("Averages"),
                    value: format!(
                        "1m {}  \u{00b7}  5m {}  \u{00b7}  15m {}",
                        format_percent(cpu.average_1m_percent),
                        format_percent(cpu.average_5m_percent),
                        format_percent(cpu.average_15m_percent)
                    ),
                });
            }
        }
    }

    if let Some(memory) = snapshot.memory.as_ref() {
        if overview || focus == Some(Focus::Memory) {
            let detail = format!(
                "{} of {} \u{00b7} commit {}",
                format_bytes(memory.used_bytes, units),
                format_bytes(memory.total_bytes, units),
                format_bytes(memory.commit_bytes, units)
            );
            rows.push(Row {
                label: "Memory",
                value: format_percent(memory.percent()),
                detail,
                series: series(&history.memory, memory.percent()),
                current: memory.percent(),
                warn: 85.0,
                critical: 95.0,
            });

            if focus == Some(Focus::Memory) && memory.cached_bytes > 0 {
                detail_rows.push(DetailRow {
                    label: String::from("Cached"),
                    value: format_bytes(memory.cached_bytes, units),
                });
            }
        }
    }

    if let Some(gpu) = snapshot.gpu.as_ref() {
        if overview || focus == Some(Focus::Gpu) {
            let mut detail = if gpu.vram_total_bytes > 0 {
                format!(
                    "VRAM {} of {}",
                    format_bytes(gpu.vram_used_bytes, units),
                    format_bytes(gpu.vram_total_bytes, units)
                )
            } else {
                String::from("VRAM unavailable")
            };
            if let Some(celsius) = gpu.temperature_c {
                detail.push_str(&format!(" \u{00b7} {}", format_temperature(celsius)));
            }
            if let Some(fan) = gpu.fan_percent {
                detail.push_str(&format!(" \u{00b7} fan {fan:.0}%"));
            }
            if let Some(power) = gpu.power_w {
                detail.push_str(&format!(" \u{00b7} {power:.0} W"));
            }
            rows.push(Row {
                label: "GPU",
                value: format_percent(gpu.utilization_percent),
                detail,
                series: series(&history.gpu, gpu.utilization_percent),
                current: gpu.utilization_percent,
                warn: 90.0,
                critical: 98.0,
            });

            if focus == Some(Focus::Gpu) {
                if let Some(name) = gpu.name.as_ref() {
                    detail_rows.push(DetailRow {
                        label: String::from("Adapter"),
                        value: name.clone(),
                    });
                }
                let mut clocks = Vec::new();
                if let Some(core) = gpu.core_mhz {
                    clocks.push(format!("{} core", format_frequency(core)));
                }
                if let Some(memory) = gpu.memory_mhz {
                    clocks.push(format!("{} memory", format_frequency(memory)));
                }
                if !clocks.is_empty() {
                    detail_rows.push(DetailRow {
                        label: String::from("Clocks"),
                        value: clocks.join(" \u{00b7} "),
                    });
                }
            }
        }
    }

    if let Some(disk) = snapshot.disk.as_ref() {
        if overview || focus == Some(Focus::Disk) {
            let detail = match disk.volumes.first() {
                Some(volume) => {
                    let temperature = volume
                        .temperature_c
                        .map(|celsius| format!(" \u{00b7} {}", format_temperature(celsius)))
                        .unwrap_or_default();
                    format!(
                        "{} free of {}{temperature}",
                        format_bytes(volume.free_bytes, units),
                        format_bytes(volume.total_bytes, units)
                    )
                }
                None => String::from("no volumes"),
            };
            rows.push(Row {
                label: "Disk",
                value: format_percent(disk.busy_percent),
                detail,
                series: Vec::new(),
                current: disk.busy_percent,
                warn: 90.0,
                critical: 98.0,
            });

            if focus == Some(Focus::Disk) {
                detail_rows.push(DetailRow {
                    label: String::from("Read / write"),
                    value: format!(
                        "{}  \u{00b7}  {}",
                        format_rate(disk.read_bytes_per_sec, units),
                        format_rate(disk.write_bytes_per_sec, units)
                    ),
                });
                for volume in &disk.volumes {
                    let label = if volume.label.is_empty() {
                        volume.mount.clone()
                    } else {
                        format!("{} {}", volume.mount, volume.label)
                    };
                    let temperature = volume
                        .temperature_c
                        .map(|celsius| format!(" \u{00b7} {}", format_temperature(celsius)))
                        .unwrap_or_default();
                    detail_rows.push(DetailRow {
                        label,
                        value: format!(
                            "{} used of {}{temperature}",
                            format_bytes(volume.used_bytes(), units),
                            format_bytes(volume.total_bytes, units)
                        ),
                    });
                }
            }
        }
    }

    if let Some(network) = snapshot.network.as_ref() {
        if overview || focus == Some(Focus::Network) {
            rows.push(Row {
                label: "Network",
                value: format!(
                    "\u{2193} {}  \u{2191} {}",
                    format_rate_compact(network.rx_bytes_per_sec, units),
                    format_rate_compact(network.tx_bytes_per_sec, units)
                ),
                detail: match network.adapters.first() {
                    Some(adapter) => adapter.name.clone(),
                    None => String::from("no active adapter"),
                },
                ..Row::default()
            });

            if focus == Some(Focus::Network) {
                for adapter in &network.adapters {
                    detail_rows.push(DetailRow {
                        label: adapter.name.clone(),
                        value: format!(
                            "\u{2193} {}  \u{2191} {}",
                            format_rate_short(adapter.rx_bytes_per_sec, units),
                            format_rate_short(adapter.tx_bytes_per_sec, units)
                        ),
                    });
                }
                detail_rows.push(DetailRow {
                    label: String::from("Session"),
                    value: format!(
                        "\u{2193} {}  \u{2191} {}",
                        format_bytes(network.session_rx_bytes, units),
                        format_bytes(network.session_tx_bytes, units)
                    ),
                });
            }
        }
    }

    let battery_row = snapshot.battery.as_ref().map(|battery| {
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
        if let Some(cycles) = battery.cycle_count {
            detail.push_str(&format!(" \u{00b7} {cycles} cycles"));
        }
        Row {
            label: "Battery",
            value: format_percent(battery.percent),
            detail,
            series: Vec::new(),
            ..Row::default()
        }
    });

    let system_row = snapshot.thermal.as_ref().map(|thermal| {
        let detail = match snapshot.system.as_ref() {
            Some(system) => format!(
                "Up {} \u{00b7} {} threads \u{00b7} {} handles",
                format_duration(system.uptime_secs),
                system.threads,
                system.handles
            ),
            None => String::from("ACPI thermal zone"),
        };
        Row {
            label: "System",
            value: format_temperature(thermal.system_c),
            detail,
            series: Vec::new(),
            ..Row::default()
        }
    });

    // Battery and System stay visible in every focused view. The System page
    // puts its own card first; everything else keeps Battery then System.
    if focus == Some(Focus::System) {
        rows.extend(system_row);
        rows.extend(battery_row);
    } else {
        rows.extend(battery_row);
        rows.extend(system_row);
    }

    // Only CPU and Memory ask "which app is responsible"; the other modules
    // have no per-app data, so their focused view has no process table.
    let processes = match focus {
        Some(Focus::Gpu) | Some(Focus::Disk) | Some(Focus::Network) | Some(Focus::Battery)
        | Some(Focus::System) => Vec::new(),
        _ => snapshot
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
                        memory: format_bytes(entry.working_set_bytes, units),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    };

    Model {
        header,
        rows,
        detail_rows,
        cores,
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

const DETAIL_ROW_HEIGHT: f32 = 17.0;
const CORE_CELL_HEIGHT: f32 = 28.0;

/// Number of core rows for a column count.
pub(crate) fn core_grid_rows(count: usize, columns: usize) -> usize {
    let columns = columns.max(1);
    count.div_ceil(columns)
}

/// Height needed for the core grid in physical pixels.
pub(crate) fn core_grid_height(count: usize, columns: usize) -> f32 {
    if count == 0 {
        0.0
    } else {
        core_grid_rows(count, columns) as f32 * CORE_CELL_HEIGHT + 4.0
    }
}

/// Height needed for a model.
pub fn height(model: &Model, scale: f32) -> f32 {
    let padding = space::L * scale;
    let header = 18.0 * scale;
    let row = 52.0 * scale;
    let cores = core_grid_height(model.cores.len(), 2) * scale;
    padding * 2.0
        + header
        + row * model.rows.len() as f32
        + space::S * scale
        + cores
        + model.detail_rows.len() as f32 * DETAIL_ROW_HEIGHT * scale
        + process_section(model, scale, PROCESS_ROWS)
}

/// Draw an area sparkline of `values` (0..=100) into `rect`.
///
/// One column per physical pixel with linear interpolation between samples, so
/// the curve reads smoothly even with only a minute of history.
pub(crate) fn sparkline(
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

/// Labelled per-core grid: core number, frequency, percentage and meter.
pub(crate) fn core_grid(
    surface: &mut Surface,
    text: &mut TextEngine,
    cores: &[CoreReadout],
    (x, y, width): (f32, f32, f32),
    columns: usize,
    scale: f32,
    palette: &Palette,
) {
    if cores.is_empty() || width < 40.0 {
        return;
    }
    let columns = columns.max(1);
    let gap = 10.0 * scale;
    let cell_width =
        ((width - gap * (columns.saturating_sub(1)) as f32) / columns as f32).max(20.0);
    let cell_height = CORE_CELL_HEIGHT * scale;

    for (index, core) in cores.iter().enumerate() {
        let column = index % columns;
        let row = index / columns;
        let cx = x + column as f32 * (cell_width + gap);
        let cy = y + row as f32 * cell_height;

        let label = format!("Core {}", index + 1);
        let _ = text.draw(
            surface,
            &label,
            (cx, cy, cell_width, 11.0 * scale),
            TextStyle::new(8.5 * scale, true, Align::Left),
            palette.text_muted,
        );
        let value = format_percent(core.percent);
        let _ = text.draw(
            surface,
            &value,
            (cx, cy, cell_width, 11.0 * scale),
            TextStyle::new(8.5 * scale, true, Align::Right),
            palette.value_color(core.percent, 85.0, 95.0),
        );
        if let Some(mhz) = core.mhz {
            let frequency = format_frequency(mhz);
            let _ = text.draw(
                surface,
                &frequency,
                (cx, cy + 12.0 * scale, cell_width, 9.0 * scale),
                TextStyle::new(7.0 * scale, false, Align::Left),
                palette.text_muted,
            );
        }

        let meter_y = cy + 22.0 * scale;
        surface.fill_round_rect(
            cx,
            meter_y,
            cell_width,
            3.0 * scale,
            1.5 * scale,
            palette.meter_track,
        );
        let filled = cell_width * (core.percent.clamp(0.0, 100.0) / 100.0);
        if filled > 0.5 {
            let tone = if core.percent >= 95.0 {
                palette.critical
            } else if core.percent >= 85.0 {
                palette.warning
            } else {
                palette.accent
            };
            surface.fill_round_rect(cx, meter_y, filled, 3.0 * scale, 1.5 * scale, tone);
        }
    }
}

/// Draw the panel. Returns its size in physical pixels.
///
/// `mica` makes the panel lightly translucent so the taskbar shows through a
/// little, matching the DWM backdrop on the framed windows. It is transparency,
/// not a blur: the layered window cannot host a composition backdrop.
pub fn render(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    model: &Model,
    scale: f32,
    mica: bool,
) -> (f32, f32) {
    let padding = space::L * scale;
    let width = width(scale);
    let height = height(model, scale);

    let background = if mica {
        palette.surface.with_alpha(232)
    } else {
        palette.surface
    };
    surface.begin(background);
    surface.fill_round_rect(0.0, 0.0, width, height, radius::PANEL * scale, background);

    let mut y = padding;
    let _ = text.draw(
        surface,
        &model.header,
        (padding, y, width - padding * 2.0, 16.0 * scale),
        header_style(scale),
        palette.text_muted,
    );
    y += 18.0 * scale + space::S * scale;

    for (index, row) in model.rows.iter().enumerate() {
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
            row.value_color(palette),
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
                row.chart_color(palette).scaled_alpha(0.75),
                palette.meter_track,
            );
        }

        y += row_height;

        if index == 0 {
            if !model.cores.is_empty() {
                core_grid(
                    surface,
                    text,
                    &model.cores,
                    (padding, y + 3.0 * scale, width - padding * 2.0),
                    2,
                    scale,
                    palette,
                );
                y += core_grid_height(model.cores.len(), 2) * scale;
            }

            for detail in &model.detail_rows {
                let value_width = 190.0 * scale;
                let _ = text.draw(
                    surface,
                    &detail.label,
                    (
                        padding,
                        y,
                        width - padding * 2.0 - value_width - space::S * scale,
                        14.0 * scale,
                    ),
                    process_style(scale, false),
                    palette.text_primary,
                );
                let _ = text.draw(
                    surface,
                    &detail.value,
                    (width - padding - value_width, y, value_width, 14.0 * scale),
                    process_value_style(scale),
                    palette.text_muted,
                );
                y += DETAIL_ROW_HEIGHT * scale;
            }
        }
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
            Some(Focus::Cpu) => "TOP PROCESSES \u{00b7} CPU",
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
                per_core_mhz: vec![3000, 2000],
                mhz: Some(2400),
                ..CpuSample::default()
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

        let busiest = model_focused(&snapshot, &History::default(), None, UnitSystem::Binary);
        assert_eq!(busiest.processes[0].name, "cpu.exe");
        assert_eq!(busiest.focus, None);

        let largest = model_focused(
            &snapshot,
            &History::default(),
            Some(Focus::Memory),
            UnitSystem::Binary,
        );
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
    fn focused_cpu_shows_only_cpu_plus_system_and_battery() {
        let mut snapshot = snapshot();
        snapshot.battery = Some(BatterySample {
            percent: 80.0,
            on_ac: true,
            ..Default::default()
        });
        snapshot.thermal = Some(statline_core::model::ThermalSample { system_c: 60.0 });
        snapshot.processes = Some(ProcessSample {
            total: 1,
            top: vec![ProcessEntry {
                pid: 9,
                name: String::from("busy.exe"),
                cpu_percent: Some(12.0),
                working_set_bytes: 1_000,
            }],
        });

        let model = model_focused(
            &snapshot,
            &History::default(),
            Some(Focus::Cpu),
            UnitSystem::Binary,
        );
        let labels: Vec<&str> = model.rows.iter().map(|row| row.label).collect();
        assert_eq!(labels, vec!["CPU", "Battery", "System"]);
        assert_eq!(model.cores.len(), 2, "per-core values feed the grid");
        assert_eq!(model.cores[0].percent, 10.0);
        assert_eq!(model.cores[0].mhz, Some(3000));
        assert_eq!(model.cores[1].mhz, Some(2000));
        assert_eq!(model.processes.len(), 1);
        assert_eq!(model.detail_rows.len(), 1);
        assert_eq!(model.detail_rows[0].label, "Averages");
    }

    #[test]
    fn focused_disk_lists_volumes_without_a_process_table() {
        let mut snapshot = snapshot();
        snapshot.disk = Some(statline_core::model::DiskSample {
            read_bytes_per_sec: 1.0,
            write_bytes_per_sec: 2.0,
            busy_percent: 3.0,
            volumes: vec![
                statline_core::model::VolumeSample {
                    mount: String::from("C:"),
                    label: String::from("System"),
                    free_bytes: 40,
                    total_bytes: 100,
                    temperature_c: Some(41.0),
                },
                statline_core::model::VolumeSample {
                    mount: String::from("D:"),
                    label: String::new(),
                    free_bytes: 10,
                    total_bytes: 100,
                    temperature_c: None,
                },
            ],
        });
        snapshot.processes = Some(ProcessSample {
            total: 1,
            top: vec![ProcessEntry {
                pid: 9,
                name: String::from("busy.exe"),
                cpu_percent: Some(12.0),
                working_set_bytes: 1_000,
            }],
        });

        let model = model_focused(
            &snapshot,
            &History::default(),
            Some(Focus::Disk),
            UnitSystem::Binary,
        );
        assert_eq!(model.detail_rows.len(), 3);
        assert_eq!(model.detail_rows[0].label, "Read / write");
        assert_eq!(model.detail_rows[1].label, "C: System");
        assert!(model.detail_rows[1].value.contains("41 °C"));
        assert!(
            model.processes.is_empty(),
            "disk detail has no per-app data"
        );
        assert_eq!(model.rows[0].label, "Disk");
    }

    #[test]
    fn row_colours_follow_the_thresholds() {
        let palette = Palette::dark(Color::rgb(0x00, 0x78, 0xD4));
        let mut row = Row {
            current: 50.0,
            warn: 85.0,
            critical: 95.0,
            ..Row::default()
        };
        assert_eq!(row.value_color(&palette), palette.text_primary);
        row.current = 90.0;
        assert_eq!(row.value_color(&palette), palette.warning);
        row.current = 99.0;
        assert_eq!(row.value_color(&palette), palette.critical);
    }

    #[test]
    fn core_grid_rows_round_up() {
        assert_eq!(core_grid_rows(8, 2), 4);
        assert_eq!(core_grid_rows(8, 4), 2);
        assert_eq!(core_grid_rows(5, 4), 2);
        assert_eq!(core_grid_rows(0, 2), 0);
        assert!(core_grid_height(8, 2) > core_grid_height(8, 4));
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
