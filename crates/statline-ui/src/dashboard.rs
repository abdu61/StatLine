//! The companion window: a full overview of every enabled module.
//!
//! Same renderer and model as the flyout, laid out as a two-column card grid so
//! it can hold more detail than the panel that opens over the taskbar.

use crate::flyout::{Model, Row};
use statline_render::{Align, Palette, Surface, TextEngine, TextStyle, radius, space};

fn label_style(scale: f32) -> TextStyle {
    TextStyle::new(10.0 * scale, true, Align::Left)
}

fn value_style(scale: f32) -> TextStyle {
    TextStyle::new(22.0 * scale, true, Align::Right)
}

fn detail_style(scale: f32) -> TextStyle {
    TextStyle::new(10.0 * scale, false, Align::Left)
}

fn title_style(scale: f32) -> TextStyle {
    TextStyle::new(16.0 * scale, true, Align::Left)
}

fn footer_style(scale: f32) -> TextStyle {
    TextStyle::new(9.5 * scale, false, Align::Left)
}

fn process_style(scale: f32) -> TextStyle {
    TextStyle::new(9.5 * scale, false, Align::Left)
}

fn process_value_style(scale: f32) -> TextStyle {
    TextStyle::new(9.5 * scale, false, Align::Right)
}

const GAP: f32 = 14.0;
const CARD_HEIGHT: f32 = 84.0;
/// Process rows the companion shows, in two columns.
const PROCESS_ROWS: usize = 8;
const PROCESS_LINE: f32 = 20.0;

pub fn width(scale: f32) -> f32 {
    720.0 * scale
}

fn process_section(model: &Model, scale: f32) -> f32 {
    let count = model.processes.len().min(PROCESS_ROWS);
    if count == 0 {
        return 0.0;
    }
    let lines = count.div_ceil(2) as f32;
    (GAP + 1.0 + 6.0 + 12.0 + 8.0 + lines * PROCESS_LINE) * scale
}

pub fn height(model: &Model, scale: f32) -> f32 {
    let rows = model.rows.len().div_ceil(2).max(1) as f32;
    (28.0 + 34.0 + rows * (CARD_HEIGHT + GAP) + 40.0) * scale + process_section(model, scale)
}

fn card(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    row: &Row,
    scale: f32,
    (x, y, w, h): (f32, f32, f32, f32),
) {
    let pad = space::XL * scale;
    surface.fill_round_rect(x, y, w, h, radius::CARD * scale, palette.hover);

    let _ = text.draw(
        surface,
        row.label,
        (x + pad, y + pad * 0.7, w - pad * 2.0, 16.0 * scale),
        label_style(scale),
        palette.text_muted,
    );
    let _ = text.draw(
        surface,
        &row.value,
        (x + w / 2.0, y + pad * 0.55, w / 2.0 - pad, 28.0 * scale),
        value_style(scale),
        palette.text_primary,
    );
    let _ = text.draw(
        surface,
        &row.detail,
        (x + pad, y + 44.0 * scale, w - pad * 2.0, 20.0 * scale),
        detail_style(scale),
        palette.text_muted,
    );
}

/// Draw the window. Returns its size in physical pixels.
pub fn render(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    model: &Model,
    scale: f32,
) -> (f32, f32) {
    let width = width(scale);
    let height = height(model, scale);
    let pad = 28.0 * scale;

    surface.begin(palette.surface);
    surface.fill_round_rect(
        0.0,
        0.0,
        width,
        height,
        radius::PANEL * scale,
        palette.surface,
    );

    let _ = text.draw(
        surface,
        "Statline",
        (pad, pad * 0.6, width - pad * 2.0, 24.0 * scale),
        title_style(scale),
        palette.text_primary,
    );
    let _ = text.draw(
        surface,
        &model.header,
        (
            pad,
            pad * 0.6 + 20.0 * scale,
            width - pad * 2.0,
            16.0 * scale,
        ),
        footer_style(scale),
        palette.text_muted,
    );

    let top = pad * 0.6 + 40.0 * scale + 12.0 * scale;
    let card_width = (width - pad * 2.0 - GAP * scale) / 2.0;

    for (index, row) in model.rows.iter().enumerate() {
        let column = index % 2;
        let line = index / 2;
        let x = pad + column as f32 * (card_width + GAP * scale);
        let y = top + line as f32 * (CARD_HEIGHT + GAP) * scale;
        card(
            surface,
            text,
            palette,
            row,
            scale,
            (x, y, card_width, CARD_HEIGHT * scale),
        );
    }

    if !model.processes.is_empty() {
        let lines = model.rows.len().div_ceil(2).max(1) as f32;
        let mut y = top + lines * (CARD_HEIGHT + GAP) * scale + GAP * scale;

        surface.fill_rect(pad, y, width - pad * 2.0, 1.0, palette.surface_border);
        y += 1.0 + 6.0 * scale;
        let _ = text.draw(
            surface,
            "TOP PROCESSES",
            (pad, y, width - pad * 2.0, 12.0 * scale),
            label_style(scale),
            palette.text_muted,
        );
        y += 12.0 * scale + 8.0 * scale;

        let count = model.processes.len().min(PROCESS_ROWS);
        let memory_width = 82.0 * scale;
        let cpu_width = 42.0 * scale;
        let inner_gap = space::S * scale;
        let name_width = card_width - memory_width - cpu_width - inner_gap * 2.0;

        // A hairline between the two process columns keeps the right column's
        // name from reading as part of the left column's memory value.
        let line_count = count.div_ceil(2);
        if line_count > 1 {
            surface.fill_rect(
                pad + card_width + GAP * scale / 2.0,
                y,
                1.0,
                line_count as f32 * PROCESS_LINE * scale,
                palette.surface_border,
            );
        }

        for index in 0..count {
            let process = &model.processes[index];
            let column = index % 2;
            let line = index / 2;
            let x = pad + column as f32 * (card_width + GAP * scale);
            let row_y = y + line as f32 * PROCESS_LINE * scale;

            let _ = text.draw(
                surface,
                &process.name,
                (x, row_y, name_width, 14.0 * scale),
                process_style(scale),
                palette.text_primary,
            );
            let _ = text.draw(
                surface,
                &process.cpu,
                (
                    x + card_width - memory_width - inner_gap - cpu_width,
                    row_y,
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
                    x + card_width - memory_width,
                    row_y,
                    memory_width,
                    14.0 * scale,
                ),
                process_value_style(scale),
                palette.text_muted,
            );
        }
    }

    let _ = text.draw(
        surface,
        "Settings arrive with the next milestone \u{00b7} Esc closes this window",
        (pad, height - pad * 0.9, width - pad * 2.0, 14.0 * scale),
        footer_style(scale),
        palette.text_muted.scaled_alpha(0.8),
    );

    surface.end();
    (width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flyout::ProcessRow;

    #[test]
    fn height_grows_with_rows() {
        let row = Row {
            label: "CPU",
            value: String::from("1%"),
            detail: String::new(),
            series: Vec::new(),
        };
        // Two columns: rows 1 and 2 share a line, so growth starts at the third.
        let two = Model {
            header: String::new(),
            rows: vec![row.clone(), row.clone()],
            processes: Vec::new(),
        };
        let three = Model {
            header: String::new(),
            rows: vec![row.clone(), row.clone(), row],
            processes: Vec::new(),
        };
        assert!(height(&three, 1.0) > height(&two, 1.0));
    }

    #[test]
    fn process_section_adds_height() {
        let row = Row {
            label: "CPU",
            value: String::from("1%"),
            detail: String::new(),
            series: Vec::new(),
        };
        let plain = Model {
            header: String::new(),
            rows: vec![row],
            processes: Vec::new(),
        };
        let mut with_processes = plain.clone();
        with_processes.processes = vec![ProcessRow::default(); PROCESS_ROWS];

        let expected_lines = PROCESS_ROWS.div_ceil(2) as f32;
        assert_eq!(
            height(&with_processes, 1.0) - height(&plain, 1.0),
            (GAP + 1.0 + 6.0 + 12.0 + 8.0) + expected_lines * PROCESS_LINE
        );
    }
}
