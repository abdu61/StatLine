//! Hosts the taskbar strip window and keeps it in sync with the sampler.
//!
//! Everything here runs on the UI thread. The host is created once, boxed, and
//! published through a process-wide pointer that the strip's window procedure
//! reads on every timer tick and mouse message.

use crate::dashboard_host::Dashboard;
use crate::flyout_host::Flyout;
use statline_config::{Settings, StripPlacement};
use statline_core::model::Snapshot;
use statline_metrics::thread::SamplerThread;
use statline_render::{Color, Palette, Renderer, Surface};
use statline_ui::{flyout, render, strip};
use statline_win::menus::MenuState;
use statline_win::{HWND, LPARAM, LRESULT, WPARAM, menus, strip_window, taskbar};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Duration;
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_COMMAND};
use windows::core::w;

/// Pointer to the single host instance, read by the window procedure.
///
/// The UI thread is the only writer and reader, so a relaxed atomic is enough
/// and avoids a lock on the paint path.
static HOST: AtomicPtr<Host> = AtomicPtr::new(std::ptr::null_mut());

const TIMER_ID: usize = 1;

/// `WM_MOUSELEAVE` (winuser.h); not generated in this binding.
const WM_MOUSELEAVE: u32 = 0x02A3;

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
    user_hidden: bool,
    paused: bool,
    hovered: Option<usize>,
    segment_rects: Vec<(f32, f32)>,
    tracking_leave: bool,
    flyout: Flyout,
    last_rect: Option<RECT>,
    dashboard: Dashboard,
    /// `true` when the strip is a child of `Shell_TrayWnd` (ADR tier 1).
    parented: bool,
    /// Last detail-surface request, so transitions are logged once.
    details_visible: bool,
    /// Which side of the taskbar the strip is anchored to.
    placement: taskbar::Placement,
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
        let height = ((geometry.height() as f32) - 14.0 * scale).max(14.0);

        let renderer = Renderer::new()?;
        let sampler = SamplerThread::start(Duration::from_millis(1000));
        let (hwnd, parented) = create_window(height)?;

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
            user_hidden: false,
            paused: false,
            hovered: None,
            segment_rects: Vec::new(),
            tracking_leave: false,
            flyout: Flyout::new(palette, scale),
            last_rect: None,
            dashboard: Dashboard::new(palette, scale),
            parented,
            details_visible: false,
            placement: taskbar::Placement::Right,
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
        self.set_visible(!should_hide && !self.user_hidden);
        if should_hide {
            return;
        }

        // Detail sensors and the process table only run while a surface that
        // shows them is open; the strip needs neither.
        let details_visible = self.flyout.is_open() || self.dashboard.is_open();
        if details_visible != self.details_visible {
            self.details_visible = details_visible;
            log::info!(
                "detail sampling {}",
                if details_visible { "on" } else { "off" }
            );
            self.sampler.set_detail_surfaces(details_visible);
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

        let palette = self.active_palette();
        if let Some(surface) = self.surface.as_mut() {
            let metrics = render::render(
                surface,
                &mut self.renderer.text,
                &palette,
                &segments,
                self.hovered,
                self.scale,
                self.height,
            );
            self.segment_rects = metrics.segments;
        }

        self.flyout.record(&snapshot);
        if self.flyout.is_open() {
            self.flyout.refresh(&mut self.renderer, &snapshot);
        }
        self.dashboard
            .refresh(&mut self.renderer, &snapshot, self.flyout.history());

        let padding = (4.0 * self.scale) as i32;
        let screen_left = geometry.strip_left_for(width as i32, padding, self.placement);
        let screen_top =
            geometry.rect.top + ((geometry.height() as f32 - self.height) / 2.0) as i32;

        let rect = RECT {
            left: screen_left,
            top: screen_top,
            right: screen_left + width as i32,
            bottom: screen_top + self.height as i32,
        };
        // Moving the window on every tick made it twitch and swallowed clicks:
        // the mouse-down landed on one frame and the button-up on the next.
        let moved = self
            .last_rect
            .map(|previous| {
                previous.left != rect.left
                    || previous.top != rect.top
                    || previous.right != rect.right
                    || previous.bottom != rect.bottom
            })
            .unwrap_or(true);
        if moved {
            self.last_rect = Some(rect);
            if self.parented {
                // Child windows are positioned in the taskbar's client area.
                strip_window::move_window(
                    self.hwnd,
                    strip_window::parent_client_rect(geometry.taskbar, rect),
                );
            } else {
                strip_window::move_window(self.hwnd, rect);
                strip_window::keep_on_top(self.hwnd);
            }
        }

        let mut presented = true;
        if let Some(surface) = self.surface.as_ref() {
            // A null destination in UPDATELAYEREDWINDOWINFO makes Windows use
            // the window's own position.
            presented = surface.present_layered(self.hwnd, 0, 0);
            if !self.logged_first_frame {
                self.logged_first_frame = true;
                log::info!(
                    "strip {}x{} at ({screen_left},{screen_top}) presented={presented} tier={}",
                    surface.width(),
                    surface.height(),
                    if self.parented { "child" } else { "topmost" }
                );
            }
        }

        // A layered child can still refuse ULW on some shell builds. Revert to
        // the topmost tier rather than leave an invisible strip attached.
        if !presented && self.parented {
            log::warn!("parented strip cannot present; reverting to topmost");
            strip_window::detach(self.hwnd);
            self.parented = false;
            strip_window::move_window(self.hwnd, rect);
            strip_window::keep_on_top(self.hwnd);
            if let Some(surface) = self.surface.as_ref() {
                surface.present_layered(self.hwnd, 0, 0);
            }
        }
    }

    /// Destroy and recreate the strip after Explorer restarts.
    fn recreate(&mut self) {
        log::info!("taskbar restarted; recreating strip");
        strip_window::destroy(self.hwnd);
        match create_window(self.height) {
            Ok((hwnd, parented)) => {
                self.hwnd = hwnd;
                self.parented = parented;
                self.surface = None;
                self.logged_first_frame = false;
                self.visible = true;
                self.hovered = None;
                self.segment_rects.clear();
                self.last_rect = None;
                self.tick();
            }
            Err(error) => log::error!("could not recreate strip: {error}"),
        }
    }

    /// Palette to draw with: dimmed while sampling is paused.
    fn active_palette(&self) -> Palette {
        let mut palette = self.palette;
        if self.paused {
            palette.text_primary = palette.text_primary.with_alpha(120);
            palette.text_muted = palette.text_muted.with_alpha(90);
            palette.accent = palette.accent.with_alpha(110);
            palette.meter_track = palette.meter_track.scaled_alpha(0.6);
        }
        palette
    }

    fn menu_state(&self) -> MenuState {
        MenuState {
            paused: self.paused,
            strip_visible: !self.user_hidden,
        }
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        self.sampler.set_paused(paused);
        self.tick();
    }

    fn toggle_strip(&mut self) {
        self.user_hidden = !self.user_hidden;
        let visible = !self.user_hidden;
        self.set_visible(visible);
        if visible {
            self.tick();
        }
    }

    fn set_hover(&mut self, x: f32) {
        let hovered = render::hit_test(&self.segment_rects, x);
        if hovered != self.hovered {
            self.hovered = hovered;
            self.tick();
        }
    }

    fn clear_hover(&mut self) {
        if self.hovered.is_some() {
            self.hovered = None;
            self.tick();
        }
    }

    fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        log::info!("strip visibility -> {visible}");
        if !visible {
            self.flyout.close();
        }
        strip_window::set_visible(self.hwnd, visible);
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        HOST.store(std::ptr::null_mut(), Ordering::Release);
    }
}

/// Current menu state, for callers outside this module.
pub fn menu_state() -> MenuState {
    host().map(|host| host.menu_state()).unwrap_or_default()
}

/// Toggle pause. Returns the new state when a strip exists.
pub fn toggle_paused() -> Option<bool> {
    let host = host()?;
    let paused = !host.paused;
    host.set_paused(paused);
    Some(paused)
}

/// Open or close the companion window.
pub fn toggle_dashboard() -> Option<bool> {
    let host = host()?;
    let snapshot = host.sampler.snapshot();
    let history_is_open = host.dashboard.is_open();
    if history_is_open {
        host.dashboard.close();
        Some(false)
    } else {
        host.dashboard
            .toggle(&mut host.renderer, &snapshot, host.flyout.history());
        Some(true)
    }
}

/// Show or hide the strip on user request.
pub fn toggle_strip() -> Option<bool> {
    let host = host()?;
    host.toggle_strip();
    Some(!host.user_hidden)
}

/// Apply settings that affect the live strip.
///
/// Called at startup and whenever the settings window changes something. The
/// palette is shared with the flyout and companion so an open surface recolours
/// on the next tick.
pub fn apply_settings(settings: &Settings, dark: bool) {
    let Some(host) = host() else {
        return;
    };
    let palette = default_palette(dark);
    host.palette = palette;
    host.flyout.set_palette(palette);
    host.dashboard.set_palette(palette);
    host.placement = match settings.general.strip_placement {
        StripPlacement::Right => taskbar::Placement::Right,
        StripPlacement::Left => taskbar::Placement::Left,
    };
    host.sampler.set_interval(Duration::from_millis(
        settings.general.update_interval_ms as u64,
    ));
    host.tick();
}

/// Destroy and rebuild the strip after Explorer restarts.
///
/// `TaskbarCreated` is broadcast to top-level windows only, and a parented
/// strip is a child, so the app's message window forwards the event here.
pub fn recreate() {
    if let Some(host) = host() {
        host.recreate();
    }
}

/// Map a strip segment label to the flyout card it focuses.
fn focus_for_label(label: &str) -> Option<flyout::Focus> {
    match label {
        "CPU" => Some(flyout::Focus::Cpu),
        "MEM" => Some(flyout::Focus::Memory),
        "GPU" => Some(flyout::Focus::Gpu),
        "DISK" => Some(flyout::Focus::Disk),
        // The network segment is unlabelled: the arrows say the direction.
        "" => Some(flyout::Focus::Network),
        _ => None,
    }
}

fn create_window(height: f32) -> windows::core::Result<(HWND, bool)> {
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

    // Tier 1: a child of the taskbar inherits the shell's z-order, so the Start
    // menu and other shell surfaces can no longer cover the strip.
    let parented = match taskbar::primary() {
        Some(geometry) if strip_window::adopt(hwnd, geometry.taskbar) => {
            log::info!("strip hosted by Shell_TrayWnd (tier 1)");
            true
        }
        Some(_) => {
            log::warn!("taskbar refused the strip; using topmost placement (tier 2)");
            false
        }
        None => false,
    };
    Ok((hwnd, parented))
}

fn host() -> Option<&'static mut Host> {
    let pointer = HOST.load(Ordering::Acquire);
    if pointer.is_null() {
        None
    } else {
        // SAFETY: the pointer is set once from a boxed host and cleared in its
        // `Drop`; every call here runs on the UI thread that owns it.
        Some(unsafe { &mut *pointer })
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
        DefWindowProcW, KillTimer, MA_NOACTIVATE, SetTimer, WM_CREATE, WM_DESTROY, WM_LBUTTONUP,
        WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_RBUTTONUP, WM_TIMER,
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
            if let Some(host) = host() {
                host.tick();
            }
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => {
            // Clicking the strip must not deactivate the user's foreground window.
            LRESULT(MA_NOACTIVATE as isize)
        }
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as i16 as f32;
            if let Some(host) = host() {
                if !host.tracking_leave {
                    host.tracking_leave = true;
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    // SAFETY: valid descriptor; asks for one WM_MOUSELEAVE.
                    unsafe {
                        let _ = TrackMouseEvent(&mut track);
                    }
                }
                host.set_hover(x);
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            if let Some(host) = host() {
                host.tracking_leave = false;
                host.clear_hover();
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            log::debug!("strip left click");
            let x = (lparam.0 & 0xFFFF) as i16 as f32;
            if let Some(host) = host() {
                if host.flyout.is_open() {
                    host.flyout.close();
                } else {
                    let anchor = strip_window::window_rect(hwnd);
                    let snapshot = host.sampler.snapshot();
                    // Which segment was clicked decides what the panel focuses.
                    let segments = strip::segments(&snapshot);
                    let focus = render::hit_test(&host.segment_rects, x)
                        .and_then(|index| segments.get(index))
                        .and_then(|segment| focus_for_label(segment.label));
                    host.flyout
                        .open(&mut host.renderer, &snapshot, anchor, focus);
                }
            }
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            let state = host().map(|host| host.menu_state()).unwrap_or_default();
            if let Err(error) = menus::show_tray_menu(hwnd, state) {
                log::warn!("strip context menu failed: {error}");
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            // The menu was created for this window; forward the action to the
            // app window that owns the command handlers.
            // SAFETY: plain window lookup and a posted message.
            unsafe {
                if let Ok(app) = FindWindowW(w!("StatlineMessageWindow"), None) {
                    let _ = PostMessageW(Some(app), WM_COMMAND, wparam, LPARAM(0));
                }
            }
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
