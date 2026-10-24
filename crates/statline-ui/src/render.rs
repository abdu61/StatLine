//! Drawing the taskbar strip.

use crate::strip::Segment;
use statline_render::{Align, Palette, Surface, TextEngine, TextStyle, space};

/// Geometry of one rendered strip.
#[derive(Debug, Clone, Default)]
pub struct StripMetrics {
    pub width: f32,
    pub height: f32,
    /// Left edge and width of each segment, in physical pixels.
    pub segments: Vec<(f32, f32)>,
}

fn label_style(scale: f32) -> TextStyle {
    TextStyle::new(9.0 * scale, true, Align::Left)
}

fn value_style(scale: f32) -> TextStyle {
    TextStyle::new(12.0 * scale, true, Align::Left)
}

fn segment_widths(text: &mut TextEngine, segments: &[Segment], scale: f32) -> Vec<f32> {
    let padding = space::S * scale;
    let min_width = 40.0 * scale;
    segments
        .iter()
        .map(|segment| {
            let label = text.measure(segment.label, label_style(scale));
            let value = text.measure(&segment.value, value_style(scale));
            (label.max(value) + padding * 2.0).max(min_width)
        })
        .collect()
}

/// Compute the strip size without drawing.
pub fn measure(text: &mut TextEngine, segments: &[Segment], scale: f32) -> StripMetrics {
    let gap = space::XXS * scale;
    let widths = segment_widths(text, segments, scale);
    let mut total = 0.0;
    for width in &widths {
        total += width + gap;
    }
    if total > 0.0 {
        total -= gap;
    }
    StripMetrics {
        width: total.max(1.0),
        height: 0.0,
        segments: Vec::new(),
    }
}

/// Measure and draw the strip. Returns the size used and each segment's rect.
pub fn render(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    segments: &[Segment],
    hovered: Option<usize>,
    scale: f32,
    height: f32,
) -> StripMetrics {
    let padding = space::S * scale;
    let gap = space::XXS * scale;
    let widths = segment_widths(text, segments, scale);

    let mut total = 0.0;
    for width in &widths {
        total += width + gap;
    }
    if total > 0.0 {
        total -= gap;
    }
    let width = total.max(1.0);

    surface.begin(palette.strip_background);

    let label_height = 11.0 * scale;
    let value_height = 15.0 * scale;
    let meter_height = (2.0 * scale).max(1.0);
    let label_top = (height * 0.14).max(1.0);
    let value_top = label_top + label_height;
    let meter_top = height - meter_height - (2.0 * scale);

    let mut rects = Vec::with_capacity(segments.len());
    let mut x = 0.0;
    for (index, (segment, segment_width)) in segments.iter().zip(widths.iter().copied()).enumerate()
    {
        // Hover reads as a soft plate behind the whole segment, the way taskbar
        // buttons highlight, but smaller and without animation.
        if hovered == Some(index) {
            surface.fill_round_rect(
                x,
                (height * 0.1).max(0.0),
                segment_width,
                height - (height * 0.2).max(0.0),
                space::XS * scale,
                palette.hover,
            );
        }

        let left = x + padding;
        let content = segment_width - padding * 2.0;

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

        rects.push((x, segment_width));
        x += segment_width + gap;
    }

    surface.end();
    StripMetrics {
        width,
        height,
        segments: rects,
    }
}

/// Index of the segment containing `x`, if any.
pub fn hit_test(rects: &[(f32, f32)], x: f32) -> Option<usize> {
    rects
        .iter()
        .position(|(left, width)| x >= *left && x < *left + *width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_test_picks_the_containing_segment() {
        let rects = vec![(0.0, 44.0), (44.0, 44.0), (88.0, 60.0)];
        assert_eq!(hit_test(&rects, 0.0), Some(0));
        assert_eq!(hit_test(&rects, 43.9), Some(0));
        assert_eq!(hit_test(&rects, 44.0), Some(1));
        assert_eq!(hit_test(&rects, 100.0), Some(2));
        assert_eq!(hit_test(&rects, 148.0), None);
        assert_eq!(hit_test(&rects, -1.0), None);
    }

    #[test]
    fn empty_rects_hit_nothing() {
        assert_eq!(hit_test(&[], 5.0), None);
    }
}
