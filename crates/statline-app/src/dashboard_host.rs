//! Hosts the companion window: a normal framed Windows app.

use statline_core::model::Snapshot;
use statline_core::units::UnitSystem;
use statline_render::{Palette, Renderer, Surface};
use statline_ui::dashboard;
use statline_ui::flyout::{self, Model};
use statline_win::{HWND, LPARAM, LRESULT, WPARAM, window};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, GetClientRect, WM_CLOSE, WM_PAINT, WM_SIZE,
};

const CLASS: &str = "StatlineCompanionWindow";

/// Logical size of the window, in DIPs. Tall enough for the card grid plus the
/// process table at 100 %.
const WINDOW_WIDTH: i32 = 760;
const WINDOW_HEIGHT: i32 = 680;

pub struct Dashboard {
    hwnd: Option<HWND>,
    surface: Option<Surface>,
    model: Model,
    units: UnitSystem,
    scale: f32,
    palette: Palette,
}

impl Dashboard {
    pub fn new(palette: Palette, scale: f32) -> Self {
        Self {
            hwnd: None,
            surface: None,
            model: Model::default(),
            units: UnitSystem::Binary,
            scale,
            palette,
        }
    }

    pub fn is_open(&self) -> bool {
        self.hwnd.is_some()
    }

    /// Recolour the companion; the next tick repaints with the new palette.
    pub fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
    }

    /// Change the byte units used by the cards.
    pub fn set_units(&mut self, units: UnitSystem) {
        self.units = units;
    }

    pub fn toggle(
        &mut self,
        renderer: &mut Renderer,
        snapshot: &Snapshot,
        history: &flyout::History,
    ) {
        if self.is_open() {
            self.close();
        } else {
            self.open(renderer, snapshot, history);
        }
    }

    fn open(&mut self, renderer: &mut Renderer, snapshot: &Snapshot, history: &flyout::History) {
        self.model = flyout::model_focused(snapshot, history, None, self.units);

        let width = (WINDOW_WIDTH as f32 * self.scale) as i32;
        let height = (WINDOW_HEIGHT as f32 * self.scale) as i32;
        match window::create_frame_window(
            CLASS,
            "Statline",
            Some(companion_proc),
            width,
            height,
            self.palette.is_dark,
        ) {
            Ok(hwnd) => {
                self.hwnd = Some(hwnd);
                COMPANION.store(self as *mut Dashboard, std::sync::atomic::Ordering::Release);
                self.render_surface(renderer, hwnd);
                log::info!("companion opened {width}x{height}");
            }
            Err(error) => log::error!("could not open companion: {error}"),
        }
    }

    pub fn refresh(
        &mut self,
        _renderer: &mut Renderer,
        snapshot: &Snapshot,
        history: &flyout::History,
    ) {
        let Some(hwnd) = self.hwnd else {
            return;
        };
        self.model = flyout::model_focused(snapshot, history, None, self.units);
        self.render_surface(_renderer, hwnd);
        // SAFETY: our window; a null rect invalidates the whole client area.
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }

    /// Render into the client area of `hwnd`.
    fn render_surface(&mut self, renderer: &mut Renderer, hwnd: HWND) {
        let mut client = RECT::default();
        // SAFETY: valid out-parameter for a live window.
        unsafe {
            let _ = GetClientRect(hwnd, &mut client);
        }
        let width = (client.right - client.left).max(1) as u32;
        let height = (client.bottom - client.top).max(1) as u32;

        let needs_surface = self
            .surface
            .as_ref()
            .map(|surface| surface.width() != width || surface.height() != height)
            .unwrap_or(true);
        if needs_surface {
            match renderer.surface(width, height) {
                Ok(surface) => self.surface = Some(surface),
                Err(error) => {
                    log::error!("could not allocate companion surface: {error}");
                    return;
                }
            }
        }

        if let Some(surface) = self.surface.as_mut() {
            // Fill the whole client area, then draw the panel on top of it.
            let scale = (width as f32 / WINDOW_WIDTH as f32).max(0.75);
            dashboard::render(
                surface,
                &mut renderer.text,
                &self.palette,
                &self.model,
                scale,
            );
        }
    }

    pub fn close(&mut self) {
        if let Some(hwnd) = self.hwnd.take() {
            // SAFETY: our window; DestroyWindow triggers WM_DESTROY on this thread.
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
            }
            log::info!("companion closed");
        }
        self.surface = None;
    }
}

static COMPANION: std::sync::atomic::AtomicPtr<Dashboard> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

unsafe extern "system" fn companion_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    _lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_SIZE => {
            // The surface is rebuilt on the next paint from the new client size.
            // SAFETY: our window.
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let pointer = COMPANION.load(std::sync::atomic::Ordering::Acquire);
            if pointer.is_null() {
                // SAFETY: still must validate the paint region.
                unsafe {
                    let mut ps = PAINTSTRUCT::default();
                    let hdc = BeginPaint(hwnd, &mut ps);
                    let _ = EndPaint(hwnd, &ps);
                    let _ = hdc;
                }
            } else {
                // The surface needs the renderer, which lives with the strip
                // host; the host repaints on the next tick. Validate here so
                // Windows does not spin on WM_PAINT.
                // SAFETY: begin/end pair.
                unsafe {
                    let mut ps = PAINTSTRUCT::default();
                    let hdc = BeginPaint(hwnd, &mut ps);
                    let dashboard = &*pointer;
                    if let Some(surface) = dashboard.surface.as_ref() {
                        let mut client = RECT::default();
                        let _ = GetClientRect(hwnd, &mut client);
                        surface.blit_to(hdc, client.right, client.bottom);
                    }
                    let _ = EndPaint(hwnd, &ps);
                    let _ = hdc;
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            // SAFETY: our window.
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_DESTROY => {
            COMPANION.store(std::ptr::null_mut(), std::sync::atomic::Ordering::Release);
            LRESULT(0)
        }
        _ => {
            // SAFETY: forwarding the exact arguments received by the procedure.
            unsafe { DefWindowProcW(hwnd, msg, wparam, _lparam) }
        }
    }
}
