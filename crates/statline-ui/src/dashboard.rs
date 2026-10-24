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

const GAP: f32 = 14.0;
const CARD_HEIGHT: f32 = 84.0;

pub fn width(scale: f32) -> f32 {
    720.0 * scale
}

pub fn height(model: &Model, scale: f32) -> f32 {
    let rows = model.rows.len().div_ceil(2).max(1) as f32;
    (28.0 + 34.0 + rows * (CARD_HEIGHT + GAP) + 28.0) * scale
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
        };
        let three = Model {
            header: String::new(),
            rows: vec![row.clone(), row.clone(), row],
        };
        assert!(height(&three, 1.0) > height(&two, 1.0));
    }
}
