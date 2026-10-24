//! Hosts the companion (dashboard) window.

use statline_core::model::Snapshot;
use statline_render::{Palette, Renderer, Surface};
use statline_ui::dashboard;
use statline_ui::flyout::{self, Model};
use statline_win::{HWND, LPARAM, LRESULT, WPARAM, strip_window};
use windows::Win32::Foundation::RECT;

const CLASS: &str = "StatlineDashboardWindow";

pub struct Dashboard {
    hwnd: Option<HWND>,
    surface: Option<Surface>,
    model: Model,
    scale: f32,
    palette: Palette,
}

impl Dashboard {
    pub fn new(palette: Palette, scale: f32) -> Self {
        Self {
            hwnd: None,
            surface: None,
            model: Model::default(),
            scale,
            palette,
        }
    }

    pub fn is_open(&self) -> bool {
        self.hwnd.is_some()
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
        self.model = flyout::model(snapshot, history);
        let width = dashboard::width(self.scale);
        let height = dashboard::height(&self.model, self.scale);

        // Centre on the primary monitor, above the taskbar.
        let screen_w = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN,
            )
        };
        let screen_h = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CYSCREEN,
            )
        };
        let left = ((screen_w as f32 - width) / 2.0).max(0.0) as i32;
        let top = ((screen_h as f32 - height) / 2.0).max(0.0) as i32;

        let rect = RECT {
            left,
            top,
            right: left + width as i32,
            bottom: top + height as i32,
        };

        match strip_window::create_popup_window(CLASS, "Statline", Some(dashboard_proc), rect) {
            Ok(hwnd) => {
                self.hwnd = Some(hwnd);
                self.paint(renderer);
                log::info!("dashboard opened {}x{}", width as i32, height as i32);
            }
            Err(error) => log::error!("could not open dashboard: {error}"),
        }
    }

    pub fn refresh(
        &mut self,
        renderer: &mut Renderer,
        snapshot: &Snapshot,
        history: &flyout::History,
    ) {
        if !self.is_open() {
            return;
        }
        self.model = flyout::model(snapshot, history);
        self.paint(renderer);
    }

    fn paint(&mut self, renderer: &mut Renderer) {
        let Some(hwnd) = self.hwnd else {
            return;
        };
        let width = dashboard::width(self.scale).ceil() as u32;
        let height = dashboard::height(&self.model, self.scale).ceil() as u32;
        let needs_surface = self
            .surface
            .as_ref()
            .map(|surface| surface.width() != width || surface.height() != height)
            .unwrap_or(true);
        if needs_surface {
            match renderer.surface(width, height) {
                Ok(surface) => self.surface = Some(surface),
                Err(error) => {
                    log::error!("could not allocate dashboard surface: {error}");
                    return;
                }
            }
        }
        if let Some(surface) = self.surface.as_mut() {
            dashboard::render(
                surface,
                &mut renderer.text,
                &self.palette,
                &self.model,
                self.scale,
            );
        }
        if let Some(surface) = self.surface.as_ref() {
            surface.present_layered(hwnd, 0, 0);
        }
    }

    pub fn close(&mut self) {
        if let Some(hwnd) = self.hwnd.take() {
            strip_window::destroy(hwnd);
            log::info!("dashboard closed");
        }
        self.surface = None;
    }
}

unsafe extern "system" fn dashboard_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, WM_DESTROY, WM_KEYDOWN, WM_PAINT,
    };

    match msg {
        WM_KEYDOWN => {
            if wparam.0 == 0x1B {
                // SAFETY: the window belongs to us; destroying it is the close path.
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_PAINT => {
            // SAFETY: standard begin/end paint pair on a layered window.
            unsafe {
                let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
                let _ = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
                let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        _ => {
            // SAFETY: forwarding the exact arguments received by the procedure.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}
