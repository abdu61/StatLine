//! Hosts the taskbar strip window and keeps it in sync with the sampler.

use statline_core::model::Snapshot;
use statline_metrics::thread::SamplerThread;
use statline_render::{Color, Palette, Renderer, Surface};
use statline_ui::{render, strip};
use statline_win::taskbar;
use statline_win::{HWND, LPARAM, LRESULT, WPARAM, strip_window};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Duration;
use windows::Win32::Foundation::RECT;

/// Pointer to the single host instance, read by the window procedure.
///
/// The UI thread is the only writer and reader, so a relaxed atomic is enough
/// and avoids a lock on the paint path.
static HOST: AtomicPtr<Host> = AtomicPtr::new(std::ptr::null_mut());

const TIMER_ID: usize = 1;

pub struct Host {
    renderer: Renderer,
    sampler: SamplerThread,
    surface: Option<Surface>,
    hwnd: HWND,
    palette: Palette,
    scale: f32,
    height: f32,
    last_signature: Option<String>,
}

impl Host {
    /// Start sampling and create the strip window. Must be called on the UI thread.
    pub fn start(palette: Palette) -> windows::core::Result<Self> {
        let geometry = taskbar::primary().ok_or_else(|| windows::core::Error::from_thread())?;
        let scale = geometry.dpi as f32 / 96.0;
        let height = ((geometry.height() as f32) - 8.0 * scale).max(16.0);

        let renderer = Renderer::new()?;
        let sampler = SamplerThread::start(Duration::from_millis(1000));

        strip_window::register_strip_class(Some(strip_proc))?;

        let rect = RECT {
            left: 0,
            top: 0,
            right: 1,
            bottom: height as i32,
        };
        let hwnd = strip_window::create_layered_child(
            geometry.taskbar,
            strip_window::STRIP_CLASS,
            "Statline",
            Some(strip_proc),
            rect,
        )?;

        let mut host = Self {
            renderer,
            sampler,
            surface: None,
            hwnd,
            palette,
            scale,
            height,
            last_signature: None,
        };
        HOST.store(&mut host as *mut Host, Ordering::Release);
        host.tick();
        Ok(host)
    }

    /// Recompute the readout, reposition and repaint when something changed.
    pub fn tick(&mut self) {
        let Some(geometry) = taskbar::primary() else {
            return;
        };
        if taskbar::is_hidden(geometry.taskbar) {
            return;
        }

        let snapshot: std::sync::Arc<Snapshot> = self.sampler.snapshot();
        let segments = strip::segments(&snapshot);
        let measured = render::measure(&mut self.renderer.text, &segments, self.scale);
        let width = measured.width.ceil().max(1.0);

        let signature = format!("{width}|{segments:?}");
        let size_changed = self
            .surface
            .as_ref()
            .map(|surface| surface.width() as f32 != width)
            .unwrap_or(true);

        if size_changed {
            match self.renderer.surface(width as u32, self.height as u32) {
                Ok(surface) => self.surface = Some(surface),
                Err(error) => {
                    log::error!("could not allocate strip surface: {error}");
                    return;
                }
            }
        }

        if let Some(surface) = self.surface.as_mut() {
            render::render(
                surface,
                &mut self.renderer.text,
                &self.palette,
                &segments,
                self.scale,
                self.height,
            );
        }

        let padding = (4.0 * self.scale) as i32;
        let left = geometry.strip_left(width as i32, padding);
        let top = geometry.rect.top + ((geometry.height() as f32 - self.height) / 2.0) as i32;

        strip_window::move_window(
            self.hwnd,
            RECT {
                left,
                top,
                right: left + width as i32,
                bottom: top + self.height as i32,
            },
        );
        // The window is a layered child: painting happens through the surface.
        // Repositioning changes the destination, so re-present every tick.
        if let Some(surface) = self.surface.as_ref() {
            surface.present_layered(self.hwnd, 0, 0);
        }

        if self.last_signature.as_deref() != Some(signature.as_str()) {
            self.last_signature = Some(signature);
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        HOST.store(std::ptr::null_mut(), Ordering::Release);
    }
}

/// Window procedure for the strip.
pub unsafe extern "system" fn strip_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{
        KillTimer, SetTimer, WM_TIMER, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_CREATE => {
            // SAFETY: no pointers; one timer for the refresh cadence.
            unsafe {
                SetTimer(Some(hwnd), TIMER_ID, 1000, None);
            }
            LRESULT(0)
        }
        WM_TIMER => {
            let pointer = HOST.load(Ordering::Acquire);
            if !pointer.is_null() {
                // SAFETY: the pointer is set once on the UI thread and cleared
                // in `Drop`; this code also runs on the UI thread.
                unsafe {
                    (*pointer).tick();
                }
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_DESTROY => {
            // SAFETY: timer belongs to this window.
            unsafe {
                let _ = KillTimer(Some(hwnd), TIMER_ID);
            }
            LRESULT(0)
        }
        _ => windows::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Default palette used until settings are wired (M4+).
pub fn default_palette(dark: bool) -> Palette {
    Palette::for_theme(dark, Color::rgb(0x4C, 0xC2, 0xFF))
}
