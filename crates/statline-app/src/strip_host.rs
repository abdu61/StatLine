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
    logged_first_frame: bool,
    visible: bool,
}

impl Host {
    /// Start sampling and create the strip window. Must be called on the UI thread.
    ///
    /// The host is boxed so its address is stable: the window procedure reads it
    /// through `HOST`, and a moved stack value would leave a dangling pointer
    /// there (that bug crashed the process on the first timer tick).
    pub fn start(palette: Palette) -> windows::core::Result<Box<Self>> {
        let geometry = taskbar::primary().ok_or_else(windows::core::Error::from_thread)?;
        let scale = geometry.dpi as f32 / 96.0;
        // Leave a small margin so the strip does not touch the taskbar edges.
        let height = ((geometry.height() as f32) - 8.0 * scale).max(16.0);

        let renderer = Renderer::new()?;
        let sampler = SamplerThread::start(Duration::from_millis(1000));

        let rect = RECT {
            left: 0,
            top: 0,
            right: 1,
            bottom: height as i32,
        };
        let hwnd = strip_window::create_strip_window(
            strip_window::STRIP_CLASS,
            "Statline",
            Some(strip_proc),
            rect,
        )?;

        let mut host = Box::new(Self {
            renderer,
            sampler,
            surface: None,
            hwnd,
            palette,
            scale,
            height,
            logged_first_frame: false,
            visible: true,
        });
        HOST.store(host.as_mut() as *mut Host, Ordering::Release);
        host.tick();
        Ok(host)
    }

    /// Recompute the readout, reposition and repaint.
    pub fn tick(&mut self) {
        let Some(geometry) = taskbar::primary() else {
            self.set_visible(false);
            return;
        };

        // Hide for anything that owns the screen: fullscreen games, video,
        // presentation mode, or an auto-hidden taskbar.
        let should_hide = taskbar::is_hidden(geometry.taskbar) || taskbar::fullscreen_active();
        self.set_visible(!should_hide);
        if should_hide {
            return;
        }

        let snapshot: std::sync::Arc<Snapshot> = self.sampler.snapshot();
        // The sampler publishes its first frame a moment after start; drawing an
        // empty snapshot would size the strip to a single pixel.
        if snapshot.taken_at.is_none() {
            return;
        }

        let segments = strip::segments(&snapshot);
        let measured = render::measure(&mut self.renderer.text, &segments, self.scale);
        let width = measured.width.ceil().max(1.0);

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
        let screen_left = geometry.strip_left(width as i32, padding);
        let screen_top =
            geometry.rect.top + ((geometry.height() as f32 - self.height) / 2.0) as i32;

        strip_window::move_window(
            self.hwnd,
            RECT {
                left: screen_left,
                top: screen_top,
                right: screen_left + width as i32,
                bottom: screen_top + self.height as i32,
            },
        );
        strip_window::keep_on_top(self.hwnd);

        if let Some(surface) = self.surface.as_ref() {
            // A null destination in UPDATELAYEREDWINDOWINFO makes Windows use
            // the window's own position.
            let presented = surface.present_layered(self.hwnd, 0, 0);
            if !self.logged_first_frame {
                self.logged_first_frame = true;
                log::info!(
                    "strip {}x{} at ({screen_left},{screen_top}) presented={presented}",
                    surface.width(),
                    surface.height()
                );
            }
        }
    }

    fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        log::info!("strip visibility -> {visible}");
        strip_window::set_visible(self.hwnd, visible);
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
        DefWindowProcW, KillTimer, SetTimer, WM_CREATE, WM_DESTROY, WM_LBUTTONUP, WM_RBUTTONUP,
        WM_TIMER,
    };

    match msg {
        WM_CREATE => {
            // SAFETY: no pointers; one timer drives the refresh cadence.
            unsafe {
                SetTimer(Some(hwnd), TIMER_ID, 1000, None);
            }
            LRESULT(0)
        }
        WM_TIMER => {
            let pointer = HOST.load(Ordering::Acquire);
            if !pointer.is_null() {
                // SAFETY: the pointer is stable for the host's lifetime and this
                // code runs on the UI thread that owns it.
                unsafe {
                    (*pointer).tick();
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            // The flyout lands in M5; until then a click is a no-op that we log
            // so the hit path is visible during development.
            log::debug!("strip clicked");
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            log::debug!("strip right-clicked");
            LRESULT(0)
        }
        WM_DESTROY => {
            // SAFETY: the timer belongs to this window.
            unsafe {
                let _ = KillTimer(Some(hwnd), TIMER_ID);
            }
            LRESULT(0)
        }
        _ => {
            // SAFETY: forwarding the exact arguments received by the procedure.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

/// Default palette used until settings are wired (M4+).
pub fn default_palette(dark: bool) -> Palette {
    Palette::for_theme(dark, Color::rgb(0x4C, 0xC2, 0xFF))
}
