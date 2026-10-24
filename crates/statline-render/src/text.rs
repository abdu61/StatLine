//! Text measurement and drawing with DirectWrite.

use crate::color::Color;
use crate::surface::Surface;
use std::collections::HashMap;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_METRICS,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat,
    IDWriteTextLayout,
};
use windows::core::{Result, w};

/// Horizontal alignment inside a rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// A font style. Sizes are in DIPs; callers scale by DPI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub semibold: bool,
    pub align: Align,
}

impl TextStyle {
    pub const fn new(size: f32, semibold: bool, align: Align) -> Self {
        Self {
            size,
            semibold,
            align,
        }
    }
}

/// Owns the DirectWrite factory and its format cache.
pub struct TextEngine {
    factory: IDWriteFactory,
    formats: HashMap<(u32, bool, Align), IDWriteTextFormat>,
}

impl TextEngine {
    pub fn new() -> Result<Self> {
        // SAFETY: shared factory, no pointers passed.
        let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        Ok(Self {
            factory,
            formats: HashMap::new(),
        })
    }

    fn format(&mut self, style: TextStyle) -> Result<IDWriteTextFormat> {
        let key = (
            (style.size * 10.0).round() as u32,
            style.semibold,
            style.align,
        );
        if let Some(format) = self.formats.get(&key) {
            return Ok(format.clone());
        }

        let weight: DWRITE_FONT_WEIGHT = if style.semibold {
            DWRITE_FONT_WEIGHT_SEMI_BOLD
        } else {
            DWRITE_FONT_WEIGHT_NORMAL
        };

        // SAFETY: the family name is a static wide string; the factory returns
        // a reference-counted format object.
        let format = unsafe {
            self.factory.CreateTextFormat(
                w!("Segoe UI Variable Text"),
                None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                style.size,
                w!("en-us"),
            )?
        };

        let alignment: DWRITE_TEXT_ALIGNMENT = match style.align {
            Align::Left => windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT_LEADING,
            Align::Center => windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT_CENTER,
            Align::Right => windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT_TRAILING,
        };

        // SAFETY: setters only mutate the format we own.
        unsafe {
            let _ = format.SetTextAlignment(alignment);
            let _ = format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
        }

        self.formats.insert(key, format.clone());
        Ok(format)
    }

    fn layout(&mut self, text: &str, style: TextStyle) -> Result<IDWriteTextLayout> {
        let format = self.format(style)?;
        let wide: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: the slice and format outlive the call; generous max size.
        unsafe {
            self.factory
                .CreateTextLayout(&wide, &format, 4096.0, 4096.0)
        }
    }

    /// Measured width of `text` in DIPs.
    pub fn measure(&mut self, text: &str, style: TextStyle) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        match self.layout(text, style) {
            Ok(layout) => {
                let mut metrics = DWRITE_TEXT_METRICS::default();
                // SAFETY: `metrics` is a valid out-parameter.
                if unsafe { layout.GetMetrics(&mut metrics) }.is_ok() {
                    metrics.widthIncludingTrailingWhitespace
                } else {
                    0.0
                }
            }
            Err(_) => 0.0,
        }
    }

    /// Draw `text` inside `rect` (physical pixels).
    pub fn draw(
        &mut self,
        surface: &mut Surface,
        text: &str,
        rect: (f32, f32, f32, f32),
        style: TextStyle,
        color: Color,
    ) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        let format = self.format(style)?;
        let wide: Vec<u16> = text.encode_utf16().collect();
        let (x, y, width, height) = rect;
        let rect = D2D_RECT_F {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        };
        let (_, brush) = surface.brush_for(color);

        // SAFETY: all descriptors are valid; the brush and format outlive the call.
        unsafe {
            let _ = surface.render_target().DrawText(
                &wide,
                &format,
                &rect,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        Ok(())
    }
}
