//! A software Direct2D surface backed by a DIB, presentable as a layered window.
//!
//! Rendering into our own DIB (rather than an HWND target) means:
//! - no Direct3D device, no swapchain, no device-loss handling;
//! - the exact same code can render headlessly to a PNG for review;
//! - per-pixel alpha works with `UpdateLayeredWindowIndirect`.

use crate::color::Color;
use crate::png;
use std::ffi::c_void;
use std::path::Path;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
    D2D1CreateFactory, ID2D1Brush, ID2D1Factory, ID2D1RenderTarget, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GetDC, HBITMAP, HDC, HGDIOBJ, ReleaseDC, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ULW_ALPHA, UPDATELAYEREDWINDOWINFO, UpdateLayeredWindowIndirect,
};
use windows::core::{Error, Interface, Result};

/// A drawing surface sized in physical pixels.
pub struct Surface {
    width: u32,
    height: u32,
    factory: ID2D1Factory,
    render: ID2D1RenderTarget,
    /// Declared after the D2D fields on purpose: Rust drops fields in order, so
    /// the render target is released before the DC and bitmap it was bound to.
    gdi: GdiSurface,
    bits: *mut u8,
    stride: usize,
    brush: ID2D1SolidColorBrush,
    brush_base: ID2D1Brush,
    drawing: bool,
}

// SAFETY: a surface is created, used and dropped on the UI thread only. The raw
// pointer refers to a DIB this struct owns; nothing else aliases it.
unsafe impl Send for Surface {}

impl Surface {
    /// Create a surface and its Direct2D target.
    pub fn new(factory: ID2D1Factory, width: u32, height: u32) -> Result<Self> {
        let (width, height) = (width.max(1), height.max(1));
        let stride = width as usize * 4;

        let header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                // Negative height requests a top-down DIB, matching render order.
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits: *mut c_void = std::ptr::null_mut();
        // SAFETY: `header` is initialized; the API allocates the pixel buffer
        // and writes its address to `bits`.
        let bitmap = unsafe {
            let screen = GetDC(None);
            let result =
                CreateDIBSection(Some(screen), &header, DIB_RGB_COLORS, &mut bits, None, 0);
            ReleaseDC(None, screen);
            result?
        };
        if bits.is_null() {
            return Err(Error::from_thread());
        }

        // SAFETY: creating a memory DC compatible with the screen.
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.is_invalid() {
            // SAFETY: `bitmap` was just created and is not used elsewhere.
            unsafe {
                let _ = DeleteObject(bitmap.into());
            }
            return Err(Error::from_thread());
        }
        // SAFETY: selecting our DIB into our DC, keeping the old object.
        let previous = unsafe { SelectObject(dc, bitmap.into()) };

        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };

        // SAFETY: `props` is a valid, fully initialized descriptor.
        let target = unsafe { factory.CreateDCRenderTarget(&props)? };
        let rect = RECT {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        // SAFETY: binding our memory DC and describing the full bitmap rect.
        unsafe { target.BindDC(dc, &rect)? };

        // SAFETY: interface inheritance is guaranteed by Direct2D.
        let render: ID2D1RenderTarget = target.cast()?;

        // SAFETY: a white brush is only a colour carrier; every draw sets colour.
        let brush = unsafe {
            render.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                None,
            )?
        };
        // SAFETY: interface inheritance is guaranteed by Direct2D.
        let brush_base: ID2D1Brush = brush.cast()?;

        Ok(Self {
            width,
            height,
            factory,
            render,
            gdi: GdiSurface {
                dc,
                bitmap,
                previous,
            },
            bits: bits.cast(),
            stride,
            brush,
            brush_base,
            drawing: false,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Start a frame and clear to `background`.
    pub fn begin(&mut self, background: Color) {
        // SAFETY: BeginDraw is paired with `end`; the target is exclusively ours.
        unsafe {
            self.render.BeginDraw();
            self.render.Clear(Some(&to_color_f(background)));
        }
        self.drawing = true;
    }

    /// Finish the frame.
    pub fn end(&mut self) {
        if !self.drawing {
            return;
        }
        // SAFETY: matches `BeginDraw` in `begin`.
        unsafe {
            let _ = self.render.EndDraw(None, None);
        }
        self.drawing = false;
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: Color) {
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let rect = D2D_RECT_F {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        };
        // SAFETY: `rect` is valid; brush is surface-owned.
        unsafe {
            self.brush.SetColor(&to_color_f(color));
            self.render.FillRectangle(&rect, &self.brush_base);
        }
    }

    pub fn fill_round_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        color: Color,
    ) {
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let rounded = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            },
            radiusX: radius.max(0.0),
            radiusY: radius.max(0.0),
        };
        // SAFETY: `rounded` is valid; brush is surface-owned.
        unsafe {
            self.brush.SetColor(&to_color_f(color));
            self.render.FillRoundedRectangle(&rounded, &self.brush_base);
        }
    }

    /// Brush pair for text: the concrete brush and its base interface.
    pub(crate) fn brush_for(&self, color: Color) -> (&ID2D1SolidColorBrush, &ID2D1Brush) {
        // SAFETY: `SetColor` only reads the colour descriptor.
        unsafe {
            self.brush.SetColor(&to_color_f(color));
        }
        (&self.brush, &self.brush_base)
    }

    pub(crate) fn render_target(&self) -> &ID2D1RenderTarget {
        &self.render
    }

    /// Present this surface into a layered window at the given screen position.
    pub fn present_layered(&self, hwnd: HWND, x: i32, y: i32) -> bool {
        let _ = (x, y);
        let source = windows::Win32::Foundation::POINT { x: 0, y: 0 };
        let size = windows::Win32::Foundation::SIZE {
            cx: self.width as i32,
            cy: self.height as i32,
        };

        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        let info = UPDATELAYEREDWINDOWINFO {
            cbSize: std::mem::size_of::<UPDATELAYEREDWINDOWINFO>() as u32,
            hdcDst: HDC::default(),
            pptDst: std::ptr::null(),
            psize: &size,
            hdcSrc: self.gdi.dc,
            pptSrc: &source,
            crKey: windows::Win32::Foundation::COLORREF(0),
            pblend: &blend,
            dwFlags: ULW_ALPHA,
            prcDirty: std::ptr::null(),
        };

        // SAFETY: every pointer in `info` refers to a live local; the surface
        // bitmap matches the described size.
        let ok = unsafe { UpdateLayeredWindowIndirect(hwnd, &info).as_bool() };
        if !ok {
            log::warn!(
                "UpdateLayeredWindowIndirect failed: {}",
                windows::core::Error::from_thread()
            );
        }
        ok
    }

    /// Straight-alpha RGBA copy of the surface, for snapshots and tests.
    #[allow(clippy::manual_checked_ops)] // explicit alpha==0 guard reads better
    pub fn pixels_rgba(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.width as usize * self.height as usize * 4];
        // SAFETY: the DIB is `stride * height` bytes and lives as long as self.
        let source =
            unsafe { std::slice::from_raw_parts(self.bits, self.stride * self.height as usize) };

        for row in 0..self.height as usize {
            for column in 0..self.width as usize {
                let source_index = row * self.stride + column * 4;
                let target_index = (row * self.width as usize + column) * 4;
                let b = source[source_index] as u32;
                let g = source[source_index + 1] as u32;
                let r = source[source_index + 2] as u32;
                let a = source[source_index + 3] as u32;
                let unpremultiply = |value: u32| {
                    if a == 0 {
                        0
                    } else {
                        ((value * 255 + a / 2) / a).min(255) as u8
                    }
                };
                out[target_index] = unpremultiply(r);
                out[target_index + 1] = unpremultiply(g);
                out[target_index + 2] = unpremultiply(b);
                out[target_index + 3] = a as u8;
            }
        }
        out
    }

    /// Write the surface to a PNG file.
    pub fn save_png(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            path,
            png::encode_rgba(self.width, self.height, &self.pixels_rgba()),
        )
    }

    /// Blit the surface into a window DC, scaled to `dest` size.
    ///
    /// Used by the companion window, which is an ordinary framed window rather
    /// than a layered one: its content is opaque, so `StretchDIBits` is enough.
    pub fn blit_to(&self, hdc: HDC, dest_width: i32, dest_height: i32) {
        if dest_width <= 0 || dest_height <= 0 {
            return;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: self.width as i32,
                biHeight: -(self.height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: the DIB is `stride * height` bytes and outlives the call; the
        // header describes exactly that layout.
        unsafe {
            windows::Win32::Graphics::Gdi::StretchDIBits(
                hdc,
                0,
                0,
                dest_width,
                dest_height,
                0,
                0,
                self.width as i32,
                self.height as i32,
                Some(self.bits.cast()),
                &info,
                DIB_RGB_COLORS,
                windows::Win32::Graphics::Gdi::SRCCOPY,
            );
        }
    }
    /// Write the surface to a PNG composited over an opaque background.
    ///
    /// Snapshots of a layered surface are mostly transparent, so reviewing them
    /// on their own shows nothing useful; this blends them the way the taskbar
    /// would.
    pub fn save_png_over(&self, path: &Path, background: Color) -> std::io::Result<()> {
        let pixels = self.pixels_rgba();
        let mut composited = vec![0u8; pixels.len()];
        for (index, pixel) in pixels.chunks(4).enumerate() {
            let alpha = pixel[3] as u32;
            let blend = |over: u8, under: u8| {
                ((over as u32 * alpha + under as u32 * (255 - alpha)) / 255) as u8
            };
            let base = index * 4;
            composited[base] = blend(pixel[0], background.r);
            composited[base + 1] = blend(pixel[1], background.g);
            composited[base + 2] = blend(pixel[2], background.b);
            composited[base + 3] = 255;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, png::encode_rgba(self.width, self.height, &composited))
    }

    /// Keep the factory alive with the surfaces it created.
    pub fn factory(&self) -> &ID2D1Factory {
        &self.factory
    }
}

/// GDI resources owned by a surface.
///
/// Kept in its own type so its `Drop` runs *after* the Direct2D fields: the
/// render target is bound to this DC, and deleting the DC first corrupts the
/// heap when D2D tears the binding down.
struct GdiSurface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}

impl Drop for GdiSurface {
    fn drop(&mut self) {
        // SAFETY: restore the previous GDI object, then release what we created.
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

fn to_color_f(color: Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.r as f32 / 255.0,
        g: color.g as f32 / 255.0,
        b: color.b as f32 / 255.0,
        a: color.a as f32 / 255.0,
    }
}

/// Create the single-threaded Direct2D factory used by every surface.
pub fn create_factory() -> Result<ID2D1Factory> {
    // SAFETY: no options pointer is passed; the factory is thread-confined.
    unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }
}
