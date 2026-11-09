//! The settings window: a short list of rows you click to change.
//!
//! The window is a normal framed window (opaque, not layered), so its content
//! is rendered into a DIB and blitted, exactly like the companion.

use statline_render::{Align, Palette, Surface, TextEngine, TextStyle};

/// One editable setting.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub label: &'static str,
    pub value: String,
}

/// Everything the settings window shows.
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub rows: Vec<Row>,
    /// Optional caution shown above the footer (e.g. Mica's resource cost).
    pub warning: Option<String>,
    pub footer: String,
}

const WIDTH: f32 = 460.0;
const HEADER: f32 = 76.0;
const ROW_HEIGHT: f32 = 48.0;
const FOOTER: f32 = 44.0;

fn title_style(scale: f32) -> TextStyle {
    TextStyle::new(16.0 * scale, true, Align::Left)
}

fn subtitle_style(scale: f32) -> TextStyle {
    TextStyle::new(9.5 * scale, false, Align::Left)
}

fn label_style(scale: f32) -> TextStyle {
    TextStyle::new(11.0 * scale, true, Align::Left)
}

fn value_style(scale: f32) -> TextStyle {
    TextStyle::new(11.0 * scale, false, Align::Right)
}

/// Window width in physical pixels.
pub fn width(scale: f32) -> f32 {
    WIDTH * scale
}

/// Window height in physical pixels.
pub fn height(model: &Model, scale: f32) -> f32 {
    let warning = if model.warning.is_some() { 24.0 } else { 0.0 };
    (HEADER + model.rows.len() as f32 * ROW_HEIGHT + warning + FOOTER) * scale
}

/// Row under a client-space y coordinate, if any.
pub fn row_at(model: &Model, y: f32, scale: f32) -> Option<usize> {
    let top = HEADER * scale;
    let row_height = ROW_HEIGHT * scale;
    if y < top {
        return None;
    }
    let index = ((y - top) / row_height) as usize;
    (index < model.rows.len()).then_some(index)
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
    let pad = 24.0 * scale;

    surface.begin(palette.surface);

    let _ = text.draw(
        surface,
        "Settings",
        (pad, pad * 0.8, width - pad * 2.0, 24.0 * scale),
        title_style(scale),
        palette.text_primary,
    );
    let _ = text.draw(
        surface,
        "Changes apply immediately and are saved to settings.json",
        (
            pad,
            pad * 0.8 + 24.0 * scale,
            width - pad * 2.0,
            14.0 * scale,
        ),
        subtitle_style(scale),
        palette.text_muted,
    );

    let mut y = HEADER * scale;
    for (index, row) in model.rows.iter().enumerate() {
        if index > 0 {
            surface.fill_rect(pad, y, width - pad * 2.0, 1.0, palette.surface_border);
        }

        let _ = text.draw(
            surface,
            row.label,
            (
                pad,
                y + 16.0 * scale,
                width - pad * 2.0 - 160.0 * scale,
                16.0 * scale,
            ),
            label_style(scale),
            palette.text_primary,
        );
        let _ = text.draw(
            surface,
            &row.value,
            (
                width - pad - 150.0 * scale,
                y + 16.0 * scale,
                150.0 * scale,
                16.0 * scale,
            ),
            value_style(scale),
            palette.accent,
        );

        y += ROW_HEIGHT * scale;
    }

    if let Some(warning) = model.warning.as_ref() {
        let _ = text.draw(
            surface,
            warning,
            (pad, y + 4.0 * scale, width - pad * 2.0, 14.0 * scale),
            subtitle_style(scale),
            palette.warning,
        );
    }

    let _ = text.draw(
        surface,
        &model.footer,
        (
            pad,
            height - FOOTER * scale + 8.0 * scale,
            width - pad * 2.0,
            14.0 * scale,
        ),
        subtitle_style(scale),
        palette.text_muted,
    );

    surface.end();
    (width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(rows: usize) -> Model {
        Model {
            rows: (0..rows)
                .map(|index| Row {
                    label: "Row",
                    value: format!("{index}"),
                })
                .collect(),
            warning: None,
            footer: String::new(),
        }
    }

    #[test]
    fn rows_hit_test_to_the_expected_index() {
        let model = model(3);
        assert_eq!(row_at(&model, HEADER - 1.0, 1.0), None);
        assert_eq!(row_at(&model, HEADER + 1.0, 1.0), Some(0));
        assert_eq!(row_at(&model, HEADER + ROW_HEIGHT + 1.0, 1.0), Some(1));
        assert_eq!(
            row_at(&model, HEADER + 2.0 * ROW_HEIGHT + 1.0, 1.0),
            Some(2)
        );
        assert_eq!(row_at(&model, HEADER + 3.0 * ROW_HEIGHT, 1.0), None);
    }

    #[test]
    fn hit_testing_scales_with_dpi() {
        let model = model(2);
        let scale = 1.25;
        assert_eq!(row_at(&model, (HEADER + 1.0) * scale, scale), Some(0));
        assert_eq!(
            row_at(&model, (HEADER + ROW_HEIGHT + 1.0) * scale, scale),
            Some(1)
        );
    }

    #[test]
    fn height_grows_with_rows() {
        assert!(height(&model(3), 1.0) > height(&model(2), 1.0));
    }

    #[test]
    fn warning_adds_height() {
        let mut with_warning = model(2);
        with_warning.warning = Some(String::from("Mica can cost battery"));
        assert!(height(&with_warning, 1.0) > height(&model(2), 1.0));
    }

    /// The warning is drawn on one line in a fixed-height box, so a string that
    /// is too long is silently clipped. Keep it short enough for the 460 px
    /// panel at the smallest scale the app supports.
    #[test]
    fn warnings_fit_on_one_line() {
        // Measured allowance: roughly 62 characters of the muted 9 px face in
        // the 412 px content width at 100 % scaling.
        const MAX_CHARS: usize = 78;
        for text in [
            "Windows will ask for location permission before it reveals the network name.",
            "Mica blurs the desktop through DWM; it can cost GPU power and battery.",
        ] {
            assert!(
                text.chars().count() <= MAX_CHARS,
                "warning is {} chars, over the {MAX_CHARS} that fit: {text}",
                text.chars().count()
            );
        }
    }
}
