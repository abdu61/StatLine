//! Design tokens: palette, type scale and spacing.
//!
//! Every visual constant the UI uses lives here so the strip, flyout and
//! dashboard stay consistent and theming is a data change, not a code change.

use crate::color::Color;
use crate::text::TextStyle;

/// Colours for one theme.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub is_dark: bool,
    /// Strip background. The taskbar shows through: this is usually transparent.
    pub strip_background: Color,
    pub text_primary: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub warning: Color,
    pub critical: Color,
    /// Panel background for flyout and dashboard.
    pub surface: Color,
    pub surface_border: Color,
    pub hover: Color,
    pub meter_track: Color,
}

impl Palette {
    pub fn dark(accent: Color) -> Self {
        Self {
            is_dark: true,
            strip_background: Color::transparent(),
            text_primary: Color::rgb(0xF5, 0xF5, 0xF5),
            text_muted: Color::rgb(0xA8, 0xA8, 0xA8),
            accent,
            warning: Color::rgb(0xFF, 0xC4, 0x3D),
            critical: Color::rgb(0xFF, 0x6B, 0x6B),
            surface: Color::rgb(0x20, 0x20, 0x20),
            surface_border: Color::rgba(0xFF, 0xFF, 0xFF, 0x1F),
            hover: Color::rgba(0xFF, 0xFF, 0xFF, 0x14),
            meter_track: Color::rgba(0xFF, 0xFF, 0xFF, 0x2E),
        }
    }

    pub fn light(accent: Color) -> Self {
        Self {
            is_dark: false,
            strip_background: Color::transparent(),
            text_primary: Color::rgb(0x1A, 0x1A, 0x1A),
            text_muted: Color::rgb(0x5C, 0x5C, 0x5C),
            accent,
            warning: Color::rgb(0xB8, 0x6E, 0x00),
            critical: Color::rgb(0xC4, 0x2B, 0x1C),
            surface: Color::rgb(0xF3, 0xF3, 0xF3),
            surface_border: Color::rgba(0x00, 0x00, 0x00, 0x1F),
            hover: Color::rgba(0x00, 0x00, 0x00, 0x0F),
            meter_track: Color::rgba(0x00, 0x00, 0x00, 0x24),
        }
    }

    pub fn for_theme(dark: bool, accent: Color) -> Self {
        if dark {
            Self::dark(accent)
        } else {
            Self::light(accent)
        }
    }

    /// Colour for a value that may cross warning/critical thresholds.
    pub fn value_color(&self, value: f32, warn: f32, critical: f32) -> Color {
        if critical > 0.0 && value >= critical {
            self.critical
        } else if warn > 0.0 && value >= warn {
            self.warning
        } else {
            self.text_primary
        }
    }

    pub fn accent_for(&self, opacity: f32) -> Color {
        self.accent.scaled_alpha(opacity)
    }
}

/// Type scale, in DIPs.
pub mod type_scale {
    use crate::text::{Align, TextStyle};

    pub fn label() -> TextStyle {
        TextStyle::new(9.0, true, Align::Left)
    }

    pub fn value() -> TextStyle {
        TextStyle::new(12.0, true, Align::Left)
    }

    pub fn small() -> TextStyle {
        TextStyle::new(10.0, false, Align::Left)
    }

    pub fn body() -> TextStyle {
        TextStyle::new(12.0, false, Align::Left)
    }

    pub fn title() -> TextStyle {
        TextStyle::new(15.0, true, Align::Left)
    }

    pub fn display() -> TextStyle {
        TextStyle::new(26.0, true, Align::Left)
    }
}

/// Spacing scale, in DIPs.
pub mod space {
    pub const XXS: f32 = 2.0;
    pub const XS: f32 = 4.0;
    pub const S: f32 = 6.0;
    pub const M: f32 = 8.0;
    pub const L: f32 = 12.0;
    pub const XL: f32 = 16.0;
    pub const XXL: f32 = 24.0;
}

/// Corner radii, in DIPs.
pub mod radius {
    pub const HOVER: f32 = 4.0;
    pub const CARD: f32 = 8.0;
    pub const PANEL: f32 = 12.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accent() -> Color {
        Color::rgb(0x00, 0x78, 0xD4)
    }

    #[test]
    fn themes_have_readable_contrast() {
        for palette in [Palette::dark(accent()), Palette::light(accent())] {
            let text = palette.text_primary.luminance();
            let background = palette.surface.luminance();
            assert!(
                (text - background).abs() > 0.3,
                "text and surface must differ in luminance"
            );
        }
    }

    #[test]
    fn thresholds_pick_expected_colours() {
        let palette = Palette::dark(accent());
        assert_eq!(palette.value_color(10.0, 85.0, 95.0), palette.text_primary);
        assert_eq!(palette.value_color(90.0, 85.0, 95.0), palette.warning);
        assert_eq!(palette.value_color(99.0, 85.0, 95.0), palette.critical);
    }

    #[test]
    fn disabled_thresholds_are_ignored() {
        let palette = Palette::dark(accent());
        assert_eq!(palette.value_color(100.0, 0.0, 0.0), palette.text_primary);
    }
}
