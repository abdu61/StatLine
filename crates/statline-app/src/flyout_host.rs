//! The flyout window: opens above the strip, closes on focus loss or Esc.

use statline_core::model::Snapshot;
use statline_core::units::UnitSystem;
use statline_render::{Palette, Renderer, Surface};
use statline_ui::flyout::{self, History, Model};
use statline_win::{HWND, LPARAM, LRESULT, WPARAM, strip_window};
use std::sync::atomic::{AtomicPtr, Ordering};
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN};

/// Pointer to the live flyout, for its window procedure.
static FLYOUT: AtomicPtr<Flyout> = AtomicPtr::new(std::ptr::null_mut());

const CLASS: &str = "StatlineFlyoutWindow";
/// How many samples each sparkline keeps (one minute at the default cadence).
const HISTORY_LEN: usize = 60;

pub struct Flyout {
    hwnd: Option<HWND>,
    surface: Option<Surface>,
    history: History,
    model: Model,
    /// Module the panel was opened from; drives the highlight and sort order.
    focus: Option<flyout::Focus>,
    units: UnitSystem,
    mica: bool,
    /// Strip rectangle the panel is anchored to, kept for height changes.
    anchor: RECT,
    scale: f32,
    palette: Palette,
    last_sample: Option<std::time::SystemTime>,
}

impl Flyout {
    pub fn new(palette: Palette, scale: f32) -> Self {
        Self {
            hwnd: None,
            surface: None,
            history: History::default(),
            model: Model::default(),
            focus: None,
            units: UnitSystem::Binary,
            mica: false,
            anchor: RECT::default(),
            scale,
            palette,
            last_sample: None,
        }
    }

    /// Series used by the sparklines, shared with the companion window.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Recolour the panel; the next tick repaints with the new palette.
    pub fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
    }

    /// Change the byte units; the next refresh rebuilds the strings.
    pub fn set_units(&mut self, units: UnitSystem) {
        self.units = units;
    }

    /// Match the Mica setting; the next paint makes the panel translucent.
    pub fn set_mica(&mut self, mica: bool) {
        self.mica = mica;
    }

    pub fn is_open(&self) -> bool {
        self.hwnd.is_some()
    }

    /// Record one sample into the sparkline history (deduplicated by timestamp).
    pub fn record(&mut self, snapshot: &Snapshot) {
        if snapshot.taken_at == self.last_sample {
            return;
        }
        self.last_sample = snapshot.taken_at;

        if let Some(cpu) = snapshot.cpu.as_ref() {
            push_capped(&mut self.history.cpu, cpu.total_percent);
        }
        if let Some(memory) = snapshot.memory.as_ref() {
            push_capped(&mut self.history.memory, memory.percent());
        }
        if let Some(gpu) = snapshot.gpu.as_ref() {
            push_capped(&mut self.history.gpu, gpu.utilization_percent);
        }
    }

    /// Open the panel above `anchor` (the strip's rect), or refresh it.
    pub fn open(
        &mut self,
        renderer: &mut Renderer,
        snapshot: &Snapshot,
        anchor: RECT,
        focus: Option<flyout::Focus>,
    ) {
        self.focus = focus;
        self.anchor = anchor;
        self.record(snapshot);
        self.model = flyout::model_focused(snapshot, &self.history, focus, self.units);

        let (left, top, width, height) = self.placement();

        if self.hwnd.is_none() {
            // Create at the final position: showing at (0,0) and moving after
            // the fact flashes in the corner and can be mistaken for "under".
            let rect = RECT {
                left,
                top,
                right: left + width,
                bottom: top + height,
            };
            match strip_window::create_popup_window(CLASS, "Statline", Some(flyout_proc), rect) {
                Ok(hwnd) => {
                    self.hwnd = Some(hwnd);
                    FLYOUT.store(self as *mut Flyout, Ordering::Release);
                    log::info!("flyout window created (focus {focus:?})");
                }
                Err(error) => {
                    log::error!("could not create flyout: {error}");
                    return;
                }
            }
        } else if let Some(hwnd) = self.hwnd {
            strip_window::move_window(
                hwnd,
                RECT {
                    left,
                    top,
                    right: left + width,
                    bottom: top + height,
                },
            );
        }

        if let Some(hwnd) = self.hwnd {
            // The taskbar is topmost too; re-assert within the topmost band
            // after creation or a move.
            strip_window::keep_on_top(hwnd);
        }
        self.paint(renderer);
    }

    /// Anchored rectangle above the strip, clamped to the screen.
    fn placement(&self) -> (i32, i32, i32, i32) {
        let width = flyout::width(self.scale).ceil() as i32;
        let height = flyout::height(&self.model, self.scale).ceil() as i32;
        let margin = (8.0 * self.scale) as i32;

        let preferred =
            self.anchor.left + ((self.anchor.right - self.anchor.left) / 2) - (width / 2);
        // SAFETY: no arguments; returns the primary display width.
        let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let min_left = margin;
        let max_left = (screen_width - width - margin).max(min_left);
        let left = preferred.clamp(min_left, max_left);
        let top = (self.anchor.top - height - margin).max(margin);

        (left, top, width, height)
    }

    /// Redraw the panel at the current size.
    pub fn paint(&mut self, renderer: &mut Renderer) {
        let Some(hwnd) = self.hwnd else {
            return;
        };
        let width = flyout::width(self.scale).ceil() as u32;
        let height = flyout::height(&self.model, self.scale).ceil() as u32;

        let needs_surface = self
            .surface
            .as_ref()
            .map(|surface| surface.width() != width || surface.height() != height)
            .unwrap_or(true);
        if needs_surface {
            match renderer.surface(width, height) {
                Ok(surface) => self.surface = Some(surface),
                Err(error) => {
                    log::error!("could not allocate flyout surface: {error}");
                    return;
                }
            }
        }

        if let Some(surface) = self.surface.as_mut() {
            flyout::render(
                surface,
                &mut renderer.text,
                &self.palette,
                &self.model,
                self.scale,
                self.mica,
            );
        }
        if let Some(surface) = self.surface.as_ref() {
            surface.present_layered(hwnd, 0, 0);
        }
    }

    /// Close and destroy the panel.
    pub fn close(&mut self) {
        if let Some(hwnd) = self.hwnd.take() {
            FLYOUT.store(std::ptr::null_mut(), Ordering::Release);
            strip_window::destroy(hwnd);
        }
        self.surface = None;
        log::info!("flyout closed");
    }

    /// Model refresh used by the panel's own timer.
    pub fn refresh(&mut self, renderer: &mut Renderer, snapshot: &Snapshot) {
        self.record(snapshot);
        self.model = flyout::model_focused(snapshot, &self.history, self.focus, self.units);

        // The process table arrives a tick or two after opening, which makes
        // the panel taller. Re-anchor so it grows upward instead of over the
        // strip (or off the old window's bottom edge).
        let Some(hwnd) = self.hwnd else {
            return;
        };
        let (left, top, width, height) = self.placement();
        let current = strip_window::window_rect(hwnd);
        if current.left != left
            || current.top != top
            || current.right != left + width
            || current.bottom != top + height
        {
            strip_window::move_window(
                hwnd,
                RECT {
                    left,
                    top,
                    right: left + width,
                    bottom: top + height,
                },
            );
            strip_window::keep_on_top(hwnd);
        }

        self.paint(renderer);
    }
}

fn push_capped(series: &mut Vec<f32>, value: f32) {
    if series.len() == HISTORY_LEN {
        series.remove(0);
    }
    series.push(value);
}

fn flyout_host() -> Option<&'static mut Flyout> {
    let pointer = FLYOUT.load(Ordering::Acquire);
    if pointer.is_null() {
        None
    } else {
        // SAFETY: set when the window is created and cleared before destroy, on
        // the same thread that dispatches these messages.
        Some(unsafe { &mut *pointer })
    }
}

unsafe extern "system" fn flyout_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, WM_ACTIVATE, WM_KEYDOWN, WM_PAINT,
    };

    match msg {
        WM_ACTIVATE => {
            // Losing activation is the natural "clicked elsewhere" signal.
            if (wparam.0 & 0xFFFF) == 0 {
                if let Some(flyout) = flyout_host() {
                    flyout.close();
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 == 0x1B {
                if let Some(flyout) = flyout_host() {
                    flyout.close();
                }
            }
            LRESULT(0)
        }

        WM_PAINT => {
            // Content lives in a layered surface; just validate the region.
            // SAFETY: standard begin/end paint pair.
            unsafe {
                let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
                let _ = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
                let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }

        _ => {
            // SAFETY: forwarding the exact arguments received by the procedure.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}
