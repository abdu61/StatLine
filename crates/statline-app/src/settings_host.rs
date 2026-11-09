//! The settings window.
//!
//! A frameless-by-content but normally framed tool window: four rows you click
//! to cycle, applied immediately and persisted atomically. It owns its own
//! renderer so it also works under `--tray-only`, where there is no strip host.

use crate::strip_host;
use statline_config::{INTERVALS_MS, Settings};
use statline_render::{Palette, Renderer, Surface};
use statline_ui::settings;
use statline_win::{HWND, LPARAM, LRESULT, WPARAM, autostart, paths, taskbar, theme, window};
use std::path::PathBuf;
use std::sync::atomic::{AtomicPtr, Ordering};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, DestroyWindow, GetClientRect, WM_CLOSE, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONUP,
    WM_PAINT, WM_SIZE,
};

const CLASS: &str = "StatLineSettingsWindow";

/// Pointer to the live window, for its own procedure.
static SETTINGS_WINDOW: AtomicPtr<SettingsWindow> = AtomicPtr::new(std::ptr::null_mut());

struct SettingsWindow {
    hwnd: HWND,
    surface: Option<Surface>,
    renderer: Renderer,
    settings: Settings,
    path: PathBuf,
    model: settings::Model,
    scale: f32,
    palette: Palette,
}

/// Open the settings window, or close it when it is already open.
pub fn toggle() {
    let pointer = SETTINGS_WINDOW.load(Ordering::Acquire);
    if !pointer.is_null() {
        // SAFETY: the pointer is the boxed window published below; the window
        // procedure drops and clears it on WM_DESTROY.
        let hwnd = unsafe { (*pointer).hwnd };
        // SAFETY: our window on this thread.
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        return;
    }

    if let Err(error) = open() {
        log::error!("could not open settings: {error}");
    }
}

fn open() -> windows::core::Result<()> {
    let path = paths::settings_file();
    let mut settings = match Settings::load_or_default(&path) {
        Ok(settings) => settings,
        Err(error) => {
            log::warn!("could not read settings: {error}");
            Settings::default()
        }
    };
    settings.normalize();
    // The Run key is the truth for startup, not a possibly stale file.
    settings.general.run_at_startup = autostart::is_enabled();

    let scale = taskbar::primary()
        .map(|geometry| geometry.dpi as f32 / 96.0)
        .unwrap_or(1.0);
    let model = build_model(&settings);
    let dark = settings
        .appearance
        .theme
        .resolve_dark(theme::is_dark_mode());
    let palette = strip_host::palette(dark, settings.appearance.accent);

    // Client area plus an allowance for the frame; render() fills the client.
    let width = settings::width(scale) as i32 + (20.0 * scale) as i32;
    let height = settings::height(&model, scale) as i32 + (44.0 * scale) as i32;

    let renderer = Renderer::new()?;
    let hwnd = window::create_frame_window(
        CLASS,
        "StatLine Settings",
        Some(settings_proc),
        window::FrameOptions {
            width,
            height,
            dark,
            mica: settings.appearance.mica,
            bounds: None,
        },
    )?;
    let mut window = Box::new(SettingsWindow {
        hwnd,
        surface: None,
        renderer,
        settings,
        path,
        model,
        scale,
        palette,
    });
    window.render();
    SETTINGS_WINDOW.store(Box::into_raw(window), Ordering::Release);
    log::info!("settings window opened");
    Ok(())
}

/// Apply the change for one settings row.
///
/// Rows are addressed by their position in [`build_model`], so the two must be
/// kept in the same order. A test walks every row and fails if its index has no
/// handler or changes nothing. Index 8 ("Start with Windows") is handled by the
/// caller because it can fail; it still reports success here so the test sees it
/// as present.
fn cycle_setting(settings: &mut Settings, index: usize) -> bool {
    match index {
        0 => settings.appearance.theme = settings.appearance.theme.next(),
        1 => {
            settings.general.update_interval_ms = next_interval(settings.general.update_interval_ms)
        }
        2 => settings.general.units = settings.general.units.next(),
        3 => settings.appearance.temperature = settings.appearance.temperature.next(),
        4 => settings.appearance.density = settings.appearance.density.next(),
        5 => settings.appearance.accent = settings.appearance.accent.next(),
        6 => settings.appearance.mica = !settings.appearance.mica,
        7 => settings.general.strip_placement = settings.general.strip_placement.next(),
        // Owned by the caller; toggling the flag here would claim success before
        // the registry write happened.
        8 => settings.general.run_at_startup = !settings.general.run_at_startup,
        9 => settings.modules.order.rotate(),
        // Also owned by the caller: it must reach the sampler as well.
        10 => settings.general.wifi_details = !settings.general.wifi_details,
        11 => settings.modules.cpu = !settings.modules.cpu,
        12 => settings.modules.memory = !settings.modules.memory,
        13 => settings.modules.gpu = !settings.modules.gpu,
        14 => settings.modules.disk = !settings.modules.disk,
        15 => settings.modules.network = !settings.modules.network,
        _ => return false,
    }
    true
}

fn on_off(value: bool) -> String {
    String::from(if value { "On" } else { "Off" })
}

fn build_model(settings: &Settings) -> settings::Model {
    settings::Model {
        rows: vec![
            settings::Row {
                label: "Theme",
                value: String::from(settings.appearance.theme.label()),
            },
            settings::Row {
                label: "Update interval",
                value: format!("{} ms", settings.general.update_interval_ms),
            },
            settings::Row {
                label: "Units",
                value: String::from(settings.general.units.label()),
            },
            settings::Row {
                label: "Temperature",
                value: String::from(settings.appearance.temperature.label()),
            },
            settings::Row {
                label: "Strip density",
                value: String::from(settings.appearance.density.label()),
            },
            settings::Row {
                label: "Accent",
                value: String::from(settings.appearance.accent.label()),
            },
            settings::Row {
                label: "Mica window effect",
                value: String::from(if settings.appearance.mica {
                    "On"
                } else {
                    "Off"
                }),
            },
            settings::Row {
                label: "Strip placement",
                value: String::from(settings.general.strip_placement.label()),
            },
            settings::Row {
                label: "Start with Windows",
                value: String::from(if settings.general.run_at_startup {
                    "On"
                } else {
                    "Off"
                }),
            },
            settings::Row {
                label: "Wi-Fi name & signal",
                value: on_off(settings.general.wifi_details),
            },
            settings::Row {
                label: "Strip order",
                value: settings.modules.order.normalized().describe(),
            },
            settings::Row {
                label: "Strip CPU",
                value: on_off(settings.modules.cpu),
            },
            settings::Row {
                label: "Strip memory",
                value: on_off(settings.modules.memory),
            },
            settings::Row {
                label: "Strip GPU",
                value: on_off(settings.modules.gpu),
            },
            settings::Row {
                label: "Strip disk",
                value: on_off(settings.modules.disk),
            },
            settings::Row {
                label: "Strip network",
                value: on_off(settings.modules.network),
            },
        ],
        warning: if settings.general.wifi_details {
            // The only setting that triggers an operating-system prompt, so it
            // has to fit one line and say the consequence before it happens.
            Some(String::from(
                "Windows will ask for location permission before it reveals the network name.",
            ))
        } else if settings.appearance.mica {
            Some(String::from(
                "Mica blurs the desktop through DWM; it can cost GPU power and battery.",
            ))
        } else {
            None
        },
        footer: String::from("Click a row to change \u{00b7} Esc closes"),
    }
}

impl SettingsWindow {
    fn click(&mut self, y: f32) {
        let Some(index) = settings::row_at(&self.model, y, self.scale) else {
            return;
        };
        if !self.cycle(index) {
            return;
        }
        if let Err(error) = self.settings.save(&self.path) {
            log::warn!("could not save settings: {error}");
        }
        self.apply();
        self.model = build_model(&self.settings);
        self.render();
        self.invalidate();
        log::info!("settings changed, saved to {}", self.path.display());
    }

    fn cycle(&mut self, index: usize) -> bool {
        // Two rows need code beyond "flip a value": the startup entry can fail
        // (a denied registry write), and Wi-Fi needs its sampler gate updated.
        // Both still have an arm in `cycle_setting` so the row-alignment test
        // sees them.
        match index {
            8 => {
                let enabled = !self.settings.general.run_at_startup;
                return match autostart::set_enabled(enabled) {
                    Ok(()) => {
                        self.settings.general.run_at_startup = enabled;
                        true
                    }
                    Err(error) => {
                        log::warn!("could not change the startup entry: {error}");
                        false
                    }
                };
            }
            10 => {
                let enabled = !self.settings.general.wifi_details;
                self.settings.general.wifi_details = enabled;
                if let Some(host) = strip_host::host() {
                    host.set_wifi_details(enabled);
                }
                return true;
            }
            _ => {}
        }
        cycle_setting(&mut self.settings, index)
    }

    fn apply(&mut self) {
        let dark = self
            .settings
            .appearance
            .theme
            .resolve_dark(theme::is_dark_mode());
        self.palette = strip_host::palette(dark, self.settings.appearance.accent);
        strip_host::apply_settings(&self.settings, dark);
        window::set_backdrop(self.hwnd, self.settings.appearance.mica);
    }

    fn render(&mut self) {
        let mut client = RECT::default();
        // SAFETY: valid out-parameter for a live window.
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut client);
        }
        let width = (client.right - client.left).max(1) as u32;
        let height = (client.bottom - client.top).max(1) as u32;

        let needs_surface = self
            .surface
            .as_ref()
            .map(|surface| surface.width() != width || surface.height() != height)
            .unwrap_or(true);
        if needs_surface {
            match self.renderer.surface(width, height) {
                Ok(surface) => self.surface = Some(surface),
                Err(error) => {
                    log::error!("could not allocate settings surface: {error}");
                    return;
                }
            }
        }

        if let Some(surface) = self.surface.as_mut() {
            settings::render(
                surface,
                &mut self.renderer.text,
                &self.palette,
                &self.model,
                self.scale,
            );
        }
    }

    fn invalidate(&self) {
        // SAFETY: our window; a null rect invalidates the whole client area.
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }
}

fn next_interval(current: u32) -> u32 {
    let index = INTERVALS_MS
        .iter()
        .position(|value| *value == current)
        .unwrap_or(0);
    INTERVALS_MS[(index + 1) % INTERVALS_MS.len()]
}

fn window_ref() -> Option<&'static mut SettingsWindow> {
    let pointer = SETTINGS_WINDOW.load(Ordering::Acquire);
    if pointer.is_null() {
        None
    } else {
        // SAFETY: the pointer is set from a boxed window and cleared in its
        // WM_DESTROY handler, on the UI thread that dispatches these messages.
        Some(unsafe { &mut *pointer })
    }
}

unsafe extern "system" fn settings_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_LBUTTONUP => {
            let y = (lparam.0 >> 16) as i16 as f32;
            if let Some(window) = window_ref() {
                window.click(y);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 == 0x1B {
                // SAFETY: our window.
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_SIZE => {
            if let Some(window) = window_ref() {
                window.render();
                window.invalidate();
            }
            LRESULT(0)
        }
        WM_PAINT => {
            // SAFETY: standard begin/end paint pair; blit our DIB if ready.
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if let Some(window) = window_ref() {
                    if let Some(surface) = window.surface.as_ref() {
                        let mut client = RECT::default();
                        let _ = GetClientRect(hwnd, &mut client);
                        surface.blit_to(hdc, client.right, client.bottom);
                    }
                }
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            // SAFETY: our window.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let pointer = SETTINGS_WINDOW.swap(std::ptr::null_mut(), Ordering::Acquire);
            if !pointer.is_null() {
                // SAFETY: the pointer came from Box::into_raw in `open`, and
                // this is the only place it is reclaimed.
                drop(unsafe { Box::from_raw(pointer) });
            }
            log::info!("settings window closed");
            LRESULT(0)
        }
        _ => {
            // SAFETY: forwarding the exact arguments received by the procedure.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_cycle_through_every_option() {
        let mut interval = INTERVALS_MS[0];
        for expected in [
            INTERVALS_MS[1],
            INTERVALS_MS[2],
            INTERVALS_MS[3],
            INTERVALS_MS[0],
        ] {
            interval = next_interval(interval);
            assert_eq!(interval, expected);
        }
    }

    #[test]
    fn unknown_interval_restarts_the_cycle() {
        assert_eq!(next_interval(137), INTERVALS_MS[1]);
    }

    /// Every row the settings page draws must have a handler at its own index.
    ///
    /// The list and the `match index` are written separately, so inserting a row
    /// without shifting the arms makes a click change the wrong setting — silent,
    /// and easy to miss in a screenshot.
    #[test]
    fn every_settings_row_has_a_handler() {
        let model = build_model(&Settings::default());
        assert!(
            !model.rows.is_empty(),
            "the settings page must draw its rows"
        );

        for index in 0..model.rows.len() {
            let mut settings = Settings::default();
            assert!(
                cycle_setting(&mut settings, index),
                "row {index} ({}) has no handler",
                model.rows[index].label
            );
            assert_ne!(
                settings,
                Settings::default(),
                "row {index} ({}) returned success but changed nothing",
                model.rows[index].label
            );
        }

        // One past the end must be rejected rather than clamping to the last row.
        let mut settings = Settings::default();
        assert!(!cycle_setting(&mut settings, model.rows.len()));
    }
}
