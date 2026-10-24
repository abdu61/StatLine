//! Colours and the design palette.

/// Straight-alpha RGBA colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn transparent() -> Self {
        Self::rgba(0, 0, 0, 0)
    }

    /// `0xRRGGBB` plus an alpha byte.
    pub const fn from_hex(value: u32, alpha: u8) -> Self {
        Self {
            r: ((value >> 16) & 0xFF) as u8,
            g: ((value >> 8) & 0xFF) as u8,
            b: (value & 0xFF) as u8,
            a: alpha,
        }
    }

    /// Same colour with a different opacity.
    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// Scale the alpha by `factor` (0.0..=1.0).
    pub fn scaled_alpha(self, factor: f32) -> Self {
        let a = (self.a as f32 * factor.clamp(0.0, 1.0)).round() as u8;
        self.with_alpha(a)
    }

    /// Mix towards `other` by `t` (0.0 = self, 1.0 = other).
    pub fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let blend = |a: u8, b: u8| {
            (a as f32 + (b as f32 - a as f32) * t)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Self {
            r: blend(self.r, other.r),
            g: blend(self.g, other.g),
            b: blend(self.b, other.b),
            a: blend(self.a, other.a),
        }
    }

    /// Relative luminance, used to pick readable foreground colours.
    pub fn luminance(self) -> f32 {
        let channel = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    /// Premultiplied components, which is what a D2D premultiplied target wants
    /// in the DIB it renders into.
    pub fn premultiplied(self) -> [u8; 4] {
        let a = self.a as u32;
        let scale = |c: u8| ((c as u32 * a + 127) / 255) as u8;
        [scale(self.b), scale(self.g), scale(self.r), self.a]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing_is_rrggbb() {
        let color = Color::from_hex(0x12_34_56, 255);
        assert_eq!(
            (color.r, color.g, color.b, color.a),
            (0x12, 0x34, 0x56, 255)
        );
    }

    #[test]
    fn premultiplication_scales_rgb_by_alpha() {
        let half = Color::rgba(255, 128, 0, 128).premultiplied();
        assert_eq!(half[3], 128);
        assert!(half[2] <= 128);
    }

    #[test]
    fn mixing_hits_both_ends() {
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        assert_eq!(black.mix(white, 0.0), black);
        assert_eq!(black.mix(white, 1.0), white);
    }

    #[test]
    fn luminance_orders_black_below_white() {
        assert!(Color::rgb(0, 0, 0).luminance() < Color::rgb(255, 255, 255).luminance());
    }

    #[test]
    fn scaled_alpha_is_clamped() {
        let color = Color::rgba(1, 2, 3, 200);
        assert_eq!(color.scaled_alpha(2.0).a, 200);
        assert_eq!(color.scaled_alpha(-1.0).a, 0);
    }
}
