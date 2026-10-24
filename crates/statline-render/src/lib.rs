//! Direct2D/DirectWrite rendering, design tokens and widget primitives.
//!
//! The whole UI renders into an ordinary DIB through a software Direct2D
//! target: no Direct3D device, no swapchain, no device-loss handling, and the
//! same code path produces PNG snapshots for review.

pub mod color;
pub mod png;
pub mod surface;
pub mod text;
pub mod tokens;

pub use color::Color;
pub use surface::{Surface, create_factory};
pub use text::{Align, TextEngine, TextStyle};
pub use tokens::{Palette, radius, space, type_scale};

/// Everything needed to draw: the device-independent factories.
pub struct Renderer {
    factory: windows::Win32::Graphics::Direct2D::ID2D1Factory,
    pub text: TextEngine,
}

impl Renderer {
    pub fn new() -> windows::core::Result<Self> {
        // SAFETY: initializing COM for this thread; `RPC_E_CHANGED_MODE` means
        // someone already initialized it differently, which is fine.
        unsafe {
            use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        Ok(Self {
            factory: create_factory()?,
            text: TextEngine::new()?,
        })
    }

    /// Allocate a surface. Recreating is cheap; surfaces are cached by callers.
    pub fn surface(&self, width: u32, height: u32) -> windows::core::Result<Surface> {
        Surface::new(self.factory.clone(), width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_roundtrips_through_premultiplication() {
        let color = Color::rgba(10, 20, 30, 255);
        let premultiplied = color.premultiplied();
        assert_eq!(premultiplied, [30, 20, 10, 255]);
    }
}
