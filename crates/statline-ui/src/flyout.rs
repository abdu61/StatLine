//! The flyout panel: a compact summary that opens above the strip.

use statline_core::model::Snapshot;
use statline_core::units::{
    UnitSystem, format_bytes, format_duration, format_percent, format_rate, format_rate_compact,
};
use statline_render::{Align, Color, Palette, Surface, TextEngine, TextStyle, radius, space};

/// Series kept for the sparklines, oldest first, 0..=100.
#[derive(Debug, Default, Clone)]
pub struct History {
    pub cpu: Vec<f32>,
    pub memory: Vec<f32>,
    pub gpu: Vec<f32>,
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

/// Everything the panel shows.
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub header: String,
    pub rows: Vec<Row>,
}

fn series(values: &[f32], latest: f32) -> Vec<f32> {
    let mut series = values.to_vec();
    if series.is_empty() {
        series.push(latest);
    }
    series
}

/// Build the panel contents from a snapshot plus the recorded history.
pub fn model(snapshot: &Snapshot, history: &History) -> Model {
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
            Some(volume) => format!(
                "{} free of {} · read {} write {}",
                format_bytes(volume.free_bytes, UnitSystem::Binary),
                format_bytes(volume.total_bytes, UnitSystem::Binary),
                format_rate(disk.read_bytes_per_sec, UnitSystem::Binary),
                format_rate(disk.write_bytes_per_sec, UnitSystem::Binary)
            ),
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
                format_rate_compact(network.rx_bytes_per_sec, UnitSystem::Binary),
                format_rate_compact(network.tx_bytes_per_sec, UnitSystem::Binary)
            ),
            detail: match network.adapters.first() {
                Some(adapter) => adapter.name.clone(),
                None => String::from("no active adapter"),
            },
            series: Vec::new(),
        });
    }

    if let Some(battery) = snapshot.battery.as_ref() {
        rows.push(Row {
            label: "Battery",
            value: format_percent(battery.percent),
            detail: match battery.seconds_left {
                Some(seconds) => format!(
                    "{} remaining{}",
                    format_duration(seconds as u64),
                    if battery.on_ac { " (charging)" } else { "" }
                ),
                None => String::from(if battery.on_ac { "on AC" } else { "on battery" }),
            },
            series: Vec::new(),
        });
    }

    Model { header, rows }
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

/// Panel width in physical pixels for a given scale.
pub fn width(scale: f32) -> f32 {
    300.0 * scale
}

/// Height needed for a model.
pub fn height(model: &Model, scale: f32) -> f32 {
    let padding = space::L * scale;
    let header = 18.0 * scale;
    let row = 52.0 * scale;
    padding * 2.0 + header + row * model.rows.len() as f32 + space::S * scale
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

        let _ = text.draw(
            surface,
            row.label,
            (padding, y, 120.0 * scale, 14.0 * scale),
            label_style(scale),
            palette.text_muted,
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

    surface.end();
    (width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use statline_core::model::{CpuSample, MemorySample};

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
        assert_eq!(height(&Model::default(), 1.0) > 0.0, true);
    }
}
