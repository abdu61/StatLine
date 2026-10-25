//! Hosts the companion window: a normal framed Windows app.

use statline_config::WindowBounds;
use statline_core::model::Snapshot;
use statline_core::units::UnitSystem;
use statline_render::{Palette, Renderer, Surface};
use statline_ui::dashboard::{self, Page};
use statline_ui::flyout::{self, Model};
use statline_win::{HWND, LPARAM, LRESULT, WPARAM, paths, window};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, GetClientRect, GetWindowRect, IsIconic, WM_CLOSE, WM_EXITSIZEMOVE,
    WM_LBUTTONUP, WM_PAINT, WM_SIZE,
};

const CLASS: &str = "StatlineCompanionWindow";

/// Logical size of the window, in DIPs. Tall enough for the card grid plus the
/// process table at 100 %.
const WINDOW_WIDTH: i32 = 880;
const WINDOW_HEIGHT: i32 = 680;

pub struct Dashboard {
    hwnd: Option<HWND>,
    surface: Option<Surface>,
    model: Model,
    units: UnitSystem,
    page: Page,
    mica: bool,
    /// Scale used by the last paint, for nav hit-testing.
    render_scale: f32,
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
            page: Page::Dashboard,
            mica: false,
            render_scale: 1.0,
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

    /// Switch page; the caller repaints with the next refresh.
    pub fn set_page(&mut self, page: Page) {
        self.page = page;
    }

    /// Enable or disable the Mica frame; applies immediately when open.
    pub fn set_mica(&mut self, mica: bool) {
        self.mica = mica;
        if let Some(hwnd) = self.hwnd {
            window::set_backdrop(hwnd, mica);
        }
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
        self.model = flyout::model_focused(snapshot, history, self.page.focus(), self.units);

        let width = (WINDOW_WIDTH as f32 * self.scale) as i32;
        let height = (WINDOW_HEIGHT as f32 * self.scale) as i32;
        let bounds = WindowBounds::load(&paths::window_state_file())
            .map(|bounds| {
                (
                    bounds.x,
                    bounds.y,
                    bounds.width as i32,
                    bounds.height as i32,
                )
            })
            .filter(|bounds| window::rect_is_visible(*bounds));
        let (log_width, log_height) = bounds
            .map(|(_, _, width, height)| (width, height))
            .unwrap_or((width, height));
        match window::create_frame_window(
            CLASS,
            "Statline",
            Some(companion_proc),
            window::FrameOptions {
                width,
                height,
                dark: self.palette.is_dark,
                mica: self.mica,
                bounds,
            },
        ) {
            Ok(hwnd) => {
                self.hwnd = Some(hwnd);
                COMPANION.store(self as *mut Dashboard, std::sync::atomic::Ordering::Release);
                self.render_surface(renderer, hwnd);
                log::info!("companion opened {log_width}x{log_height}");
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
        self.model = flyout::model_focused(snapshot, history, self.page.focus(), self.units);
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
            // Fill the whole client area, then draw the nav and page.
            let scale = (width as f32 / WINDOW_WIDTH as f32).max(0.75);
            self.render_scale = scale;
            dashboard::render(
                surface,
                &mut renderer.text,
                &self.palette,
                &self.model,
                scale,
                self.page,
            );
        }
    }

    pub fn close(&mut self) {
        if let Some(hwnd) = self.hwnd.take() {
            self.save_bounds(hwnd);
            // SAFETY: our window; DestroyWindow triggers WM_DESTROY on this thread.
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
            }
            log::info!("companion closed");
        }
        self.surface = None;
    }

    /// Persist the outer window rectangle for the next open.
    pub(crate) fn save_bounds(&self, hwnd: HWND) {
        // A minimized window reports a wild rectangle; keep the last good one.
        // SAFETY: our window.
        if unsafe { IsIconic(hwnd) }.as_bool() {
            return;
        }
        let mut rect = RECT::default();
        // SAFETY: `rect` is a valid out-parameter for a live window.
        if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
            return;
        }
        let bounds = WindowBounds {
            x: rect.left,
            y: rect.top,
            width: (rect.right - rect.left).max(1) as u32,
            height: (rect.bottom - rect.top).max(1) as u32,
        };
        if let Err(error) = bounds.save(&paths::window_state_file()) {
            log::warn!("could not save window bounds: {error}");
        }
    }
}

static COMPANION: std::sync::atomic::AtomicPtr<Dashboard> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

unsafe extern "system" fn companion_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_LBUTTONUP => {
            let x = (lparam.0 & 0xFFFF) as i16 as f32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
            let pointer = COMPANION.load(std::sync::atomic::Ordering::Acquire);
            if !pointer.is_null() {
                // SAFETY: the pointer is a live companion owned by this thread.
                let scale = unsafe { (*pointer).render_scale };
                if let Some(index) = dashboard::nav_at(x, y, scale) {
                    crate::strip_host::select_dashboard_item(index);
                }
            }
            LRESULT(0)
        }
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
        WM_EXITSIZEMOVE => {
            let pointer = COMPANION.load(std::sync::atomic::Ordering::Acquire);
            if !pointer.is_null() {
                // SAFETY: the pointer is a live companion owned by this thread.
                unsafe {
                    (*pointer).save_bounds(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let pointer = COMPANION.load(std::sync::atomic::Ordering::Acquire);
            if !pointer.is_null() {
                // SAFETY: the pointer is a live companion owned by this thread.
                unsafe {
                    (*pointer).save_bounds(hwnd);
                }
            }
            // SAFETY: our window.
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
            }
            if !pointer.is_null() {
                // SAFETY: the Dashboard outlives its window (it is owned by the
                // strip host); WM_DESTROY only clears the static pointer.
                unsafe {
                    (*pointer).hwnd = None;
                    (*pointer).surface = None;
                }
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_DESTROY => {
            COMPANION.store(std::ptr::null_mut(), std::sync::atomic::Ordering::Release);
            LRESULT(0)
        }
        _ => {
            // SAFETY: forwarding the exact arguments received by the procedure.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}
