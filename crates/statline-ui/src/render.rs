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
    /// Measured width of each segment's widest string, in physical pixels.
    /// Carried out of [`measure`] so [`render`] does not measure twice.
    pub widths: Vec<f32>,
}

fn label_style(scale: f32) -> TextStyle {
    TextStyle::new(7.5 * scale, true, Align::Left)
}

fn value_style(scale: f32) -> TextStyle {
    TextStyle::new(11.0 * scale, true, Align::Left)
}

fn stacked_style(scale: f32) -> TextStyle {
    TextStyle::new(8.5 * scale, true, Align::Left)
}

fn segment_widths(text: &mut TextEngine, segments: &[Segment], scale: f32) -> Vec<f32> {
    let padding = space::XS * scale;
    let min_width = 32.0 * scale;

    segments
        .iter()
        .map(|segment| {
            let mut widest = text.measure(segment.label, label_style(scale));
            if !segment.width_sample.is_empty() {
                widest = widest.max(text.measure(segment.width_sample, stacked_style(scale)));
            }
            widest = widest.max(text.measure(&segment.value, value_style(scale)));
            if let Some(extra) = segment.extra.as_ref() {
                widest = widest.max(text.measure(extra, stacked_style(scale)));
            }
            (widest + padding * 2.0).max(min_width)
        })
        .collect()
}

/// Compute the strip size without drawing.
pub fn measure(text: &mut TextEngine, segments: &[Segment], scale: f32) -> StripMetrics {
    let widths = segment_widths(text, segments, scale);
    StripMetrics {
        width: total_width(&widths, scale),
        height: 0.0,
        segments: Vec::new(),
        widths,
    }
}

/// Total strip width for per-segment widths, including the gaps between them.
fn total_width(widths: &[f32], scale: f32) -> f32 {
    let gap = space::XXS * scale;
    let mut total = 0.0;
    for width in widths {
        total += width + gap;
    }
    if total > 0.0 {
        total -= gap;
    }
    total.max(1.0)
}

/// Everything one strip frame needs to draw.
///
/// Grouped because the list is long and positional: a mistyped `scale` and
/// `height` would compile and draw garbage.
pub struct Frame<'a> {
    pub segments: &'a [Segment],
    /// Per-segment widths from [`measure`], so nothing is measured twice.
    pub widths: &'a [f32],
    pub hovered: Option<usize>,
    pub scale: f32,
    pub height: f32,
}

/// Measure and draw the strip. Returns the size used and each segment's rect.
///
/// Takes the widths already produced by [`measure`]: measuring a string with
/// DirectWrite is not free, and doing it twice per tick was the single largest
/// cost in the strip's 1 Hz repaint.
pub fn render(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    frame: &Frame<'_>,
) -> StripMetrics {
    let Frame {
        segments,
        widths,
        hovered,
        scale,
        height,
    } = *frame;
    let padding = space::XS * scale;
    let gap = space::XXS * scale;
    let width = total_width(widths, scale);

    surface.begin(palette.strip_background);

    let meter_height = (1.5 * scale).max(1.0);
    let mut rects = Vec::with_capacity(segments.len());
    let mut x = 0.0;

    for (index, (segment, segment_width)) in segments.iter().zip(widths.iter().copied()).enumerate()
    {
        if hovered == Some(index) {
            surface.fill_round_rect(
                x,
                height * 0.08,
                segment_width,
                height * 0.84,
                6.0 * scale,
                palette.hover,
            );
        }

        let left = x + padding;
        let content = segment_width - padding * 2.0;

        if let Some(extra) = segment.extra.as_ref() {
            // Stacked segment: two values, no label, no meter.
            let style = stacked_style(scale);
            let line_height = height * 0.34;
            let _ = text.draw(
                surface,
                &segment.value,
                (left, height * 0.14, content, line_height),
                style,
                palette.text_primary,
            );
            let _ = text.draw(
                surface,
                extra,
                (left, height * 0.52, content, line_height),
                style,
                palette.text_primary,
            );
        } else {
            let label_height = height * 0.30;
            let value_height = height * 0.40;
            let label_top = height * 0.08;
            let value_top = height * 0.34;
            let meter_top = height - meter_height - height * 0.08;

            let _ = text.draw(
                surface,
                segment.label,
                (left, label_top, content, label_height),
                label_style(scale),
                palette.text_muted,
            );
            let tone = segment
                .percent
                .map(|percent| palette.value_color(percent, segment.warn, segment.critical))
                .unwrap_or(palette.text_primary);
            let _ = text.draw(
                surface,
                &segment.value,
                (left, value_top, content, value_height),
                value_style(scale),
                tone,
            );

            if let Some(percent) = segment.percent {
                let meter_tone = if segment.critical > 0.0 && percent >= segment.critical {
                    palette.critical
                } else if segment.warn > 0.0 && percent >= segment.warn {
                    palette.warning
                } else {
                    palette.accent
                };
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
                        meter_tone,
                    );
                }
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
        widths: widths.to_vec(),
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
