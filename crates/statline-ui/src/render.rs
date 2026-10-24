//! Drawing the taskbar strip.

use crate::strip::Segment;
use statline_render::{Align, Palette, Surface, TextEngine, TextStyle, space};

/// Geometry of one rendered strip.
#[derive(Debug, Clone, Copy, Default)]
pub struct StripMetrics {
    pub width: f32,
    pub height: f32,
}

fn label_style(scale: f32) -> TextStyle {
    TextStyle::new(9.0 * scale, true, Align::Left)
}

fn value_style(scale: f32) -> TextStyle {
    TextStyle::new(12.0 * scale, true, Align::Left)
}

/// Compute the strip size without drawing.
pub fn measure(text: &mut TextEngine, segments: &[Segment], scale: f32) -> StripMetrics {
    let padding = space::S * scale;
    let gap = space::XXS * scale;
    let min_width = 40.0 * scale;

    let mut total = 0.0;
    for segment in segments {
        let label = text.measure(segment.label, label_style(scale));
        let value = text.measure(&segment.value, value_style(scale));
        total += (label.max(value) + padding * 2.0).max(min_width) + gap;
    }
    if total > 0.0 {
        total -= gap;
    }
    StripMetrics {
        width: total.max(1.0),
        height: 0.0,
    }
}

/// Measure and draw the strip. Returns the size actually used.
pub fn render(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    segments: &[Segment],
    scale: f32,
    height: f32,
) -> StripMetrics {
    let padding = space::S * scale;
    let gap = space::XXS * scale;
    let min_width = 40.0 * scale;

    let mut widths = Vec::with_capacity(segments.len());
    let mut total = 0.0;
    for segment in segments {
        let label = text.measure(segment.label, label_style(scale));
        let value = text.measure(&segment.value, value_style(scale));
        let width = (label.max(value) + padding * 2.0).max(min_width);
        total += width + gap;
        widths.push(width);
    }
    if total > 0.0 {
        total -= gap;
    }

    surface.begin(palette.strip_background);

    let label_height = 11.0 * scale;
    let value_height = 15.0 * scale;
    let meter_height = (2.0 * scale).max(1.0);
    let label_top = (height * 0.14).max(1.0);
    let value_top = label_top + label_height;
    let meter_top = height - meter_height - (2.0 * scale);

    let mut x = 0.0;
    for (segment, width) in segments.iter().zip(widths.iter().copied()) {
        let left = x + padding;
        let content = width - padding * 2.0;

        let _ = text.draw(
            surface,
            segment.label,
            (left, label_top, content, label_height),
            label_style(scale),
            palette.text_muted,
        );
        let _ = text.draw(
            surface,
            &segment.value,
            (left, value_top, content, value_height),
            value_style(scale),
            palette.text_primary,
        );

        if let Some(percent) = segment.percent {
            let filled = (content * (percent.clamp(0.0, 100.0) / 100.0)).max(0.0);
            surface.fill_round_rect(
                left,
                meter_top,
                content,
                meter_height,
                meter_height / 2.0,
                palette.meter_track,
            );
            if filled > 0.5 {
                surface.fill_round_rect(
                    left,
                    meter_top,
                    filled,
                    meter_height,
                    meter_height / 2.0,
                    palette.accent,
                );
            }
        }

        x += width + gap;
    }

    surface.end();
    StripMetrics {
        width: total.max(1.0),
        height,
    }
}
