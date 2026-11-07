//! Hosts the taskbar strip window and keeps it in sync with the sampler.
//!
//! Everything here runs on the UI thread. The host is created once, boxed, and
//! published through a process-wide pointer that the strip's window procedure
//! reads on every timer tick and mouse message.

use crate::dashboard_host::Dashboard;
use crate::flyout_host::Flyout;
use statline_config::{Accent, Density, Settings, StripPlacement, Temperature, Units};
use statline_core::model::Snapshot;
use statline_core::units::{TemperatureUnit, UnitSystem};
use statline_metrics::thread::SamplerThread;
use statline_render::{Color, Palette, Renderer, Surface};
use statline_ui::{flyout, render, strip};
use statline_win::menus::MenuState;
use statline_win::{HWND, LPARAM, LRESULT, WPARAM, menus, strip_window, taskbar, theme};
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
    /// Vertical inset from the taskbar, in device-independent pixels. Smaller
    /// means a taller strip; the density setting chooses it.
    inset: f32,
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
    /// Byte units used by the strip readouts.
    units: UnitSystem,
    /// Strip module visibility.
    modules: strip::VisibleModules,
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
        let inset = Density::default().inset();
        // Leave a small margin so the strip does not touch the taskbar edges.
        let height = strip_height(geometry.height() as f32, scale, inset);

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
            inset,
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
            units: UnitSystem::Binary,
            modules: strip::VisibleModules::default(),
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

        // Follow a DPI change (moving to another monitor, or changing the
        // scale). Without this the strip keeps the scale it started with and
        // renders blurry or too small on the new display.
        let scale = (geometry.dpi as f32 / 96.0).max(0.5);
        if (scale - self.scale).abs() > 0.001 {
            log::info!("strip scale {:.2} -> {:.2}", self.scale, scale);
            self.scale = scale;
            self.height = strip_height(geometry.height() as f32, scale, self.inset);
            self.surface = None;
            self.flyout.set_scale(scale);
            self.dashboard.set_scale(scale);
        }

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

        let segments = strip::segments_with(&snapshot, self.units, self.modules);
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

/// Switch the companion page (or open Settings) from its left nav.
pub fn select_dashboard_item(index: usize) {
    use statline_ui::dashboard::{Page, SETTINGS_ITEM};

    if index == SETTINGS_ITEM {
        crate::settings_host::toggle();
        return;
    }
    let Some(page) = Page::from_index(index) else {
        return;
    };
    let Some(host) = host() else {
        return;
    };
    host.dashboard.set_page(page);
    let snapshot = host.sampler.snapshot();
    host.dashboard
        .refresh(&mut host.renderer, &snapshot, host.flyout.history());
    log::info!("companion page: {}", page.label());
}

/// Switch the sampler to documented counters only (`--safe-mode`).
pub fn set_safe_mode(enabled: bool) {
    if let Some(host) = host() {
        host.sampler.set_safe_mode(enabled);
    }
}

/// Next/previous nav index, wrapping around the item list.
pub fn cycle_index(current: usize, count: usize, backwards: bool) -> usize {
    if count == 0 {
        return 0;
    }
    if backwards {
        (current + count - 1) % count
    } else {
        (current + 1) % count
    }
}

/// Keyboard handling for the companion window.
///
/// `Ctrl+Tab`/`Ctrl+Shift+Tab` cycle pages, `Ctrl+1..9` jump to a nav item, and
/// `Ctrl+S` opens settings. `Esc` is handled by the window itself.
pub fn handle_companion_key(key: u32, ctrl: bool, current: usize) {
    const VK_TAB: u32 = 0x09;
    const VK_S: u32 = 0x53;
    const VK_1: u32 = 0x31;
    const VK_9: u32 = 0x39;

    if !ctrl {
        return;
    }
    if key == VK_S {
        crate::settings_host::toggle();
        return;
    }
    if key == VK_TAB {
        let shift = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(0x10) < 0 };
        let count = statline_ui::dashboard::NAV_ITEMS.len();
        select_dashboard_item(cycle_index(current, count, shift));
        return;
    }
    if (VK_1..=VK_9).contains(&key) {
        select_dashboard_item((key - VK_1) as usize);
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
    let palette = palette(dark, settings.appearance.accent);
    host.palette = palette;
    host.flyout.set_palette(palette);
    host.dashboard.set_palette(palette);
    host.placement = match settings.general.strip_placement {
        StripPlacement::Right => taskbar::Placement::Right,
        StripPlacement::Left => taskbar::Placement::Left,
    };
    host.units = match settings.general.units {
        Units::Binary => UnitSystem::Binary,
        Units::Decimal => UnitSystem::Decimal,
    };
    host.modules = strip::VisibleModules {
        cpu: settings.modules.cpu,
        memory: settings.modules.memory,
        gpu: settings.modules.gpu,
        disk: settings.modules.disk,
        network: settings.modules.network,
    };
    host.flyout.set_units(host.units);
    host.flyout.set_mica(settings.appearance.mica);
    host.dashboard.set_units(host.units);
    host.dashboard.set_mica(settings.appearance.mica);

    let temperature = match settings.appearance.temperature {
        Temperature::Celsius => TemperatureUnit::Celsius,
        Temperature::Fahrenheit => TemperatureUnit::Fahrenheit,
    };
    host.flyout.set_temperature(temperature);
    host.dashboard.set_temperature(temperature);

    // A different density changes the strip height, so the surface must be
    // rebuilt and the window resized on the next tick.
    let inset = settings.appearance.density.inset();
    if (inset - host.inset).abs() > f32::EPSILON {
        host.inset = inset;
        if let Some(geometry) = taskbar::primary() {
            host.height = strip_height(geometry.height() as f32, host.scale, inset);
        }
        host.surface = None;
    }

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

/// Strip height for a taskbar of `taskbar_height`, leaving `inset` on each
/// vertical edge. Never collapses below one readable line.
fn strip_height(taskbar_height: f32, scale: f32, inset: f32) -> f32 {
    (taskbar_height - inset * scale).max(14.0)
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
                    let segments = strip::segments_with(&snapshot, host.units, host.modules);
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

/// Default accent when the system value is unavailable.
const FALLBACK_ACCENT: Color = Color::rgb(0x4C, 0xC2, 0xFF);

/// Resolve the configured accent to a concrete colour.
pub fn accent_color(accent: Accent) -> Color {
    match accent {
        Accent::System => theme::accent_rgb()
            .map(|(r, g, b)| Color::rgb(r, g, b))
            .unwrap_or(FALLBACK_ACCENT),
        Accent::Blue => FALLBACK_ACCENT,
        Accent::Green => Color::rgb(0x4A, 0xDE, 0x80),
        Accent::Purple => Color::rgb(0xA7, 0x8B, 0xFA),
        Accent::Orange => Color::rgb(0xFB, 0x92, 0x3C),
        Accent::Teal => Color::rgb(0x2D, 0xD4, 0xBF),
        Accent::Pink => Color::rgb(0xF4, 0x72, 0xB6),
    }
}

/// Palette for a theme and accent choice.
pub fn palette(dark: bool, accent: Accent) -> Palette {
    Palette::for_theme(dark, accent_color(accent))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycling_wraps_in_both_directions() {
        assert_eq!(cycle_index(0, 10, false), 1);
        assert_eq!(cycle_index(9, 10, false), 0);
        assert_eq!(cycle_index(0, 10, true), 9);
        assert_eq!(cycle_index(5, 10, true), 4);
        assert_eq!(cycle_index(3, 0, false), 0);
    }

    #[test]
    fn accent_presets_are_distinct() {
        let colors = [
            accent_color(Accent::Blue),
            accent_color(Accent::Green),
            accent_color(Accent::Purple),
            accent_color(Accent::Orange),
            accent_color(Accent::Teal),
            accent_color(Accent::Pink),
        ];
        for (index, color) in colors.iter().enumerate() {
            for other in colors.iter().skip(index + 1) {
                assert_ne!(color, other, "accent presets must differ");
            }
        }
    }
}
