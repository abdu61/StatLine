//! Context menus.

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_CHECKED, MF_SEPARATOR, MF_STRING,
    PostMessageW, SetForegroundWindow, TPM_RIGHTBUTTON, TrackPopupMenu, WM_NULL,
};
use windows::core::{PCWSTR, Result, w};

/// Open the folder that holds settings and logs.
pub const MENU_OPEN_DATA: u32 = 1;
/// Quit the application.
pub const MENU_EXIT: u32 = 2;
/// Pause or resume sampling.
pub const MENU_PAUSE: u32 = 3;
/// Show or hide the taskbar strip.
pub const MENU_TOGGLE_STRIP: u32 = 4;
/// Open the dashboard (M6).
pub const MENU_DASHBOARD: u32 = 5;
/// Open the settings window.
pub const MENU_SETTINGS: u32 = 6;

/// State the menu labels and check marks reflect.
#[derive(Debug, Clone, Copy, Default)]
pub struct MenuState {
    pub paused: bool,
    pub strip_visible: bool,
}

/// Show the tray context menu at the cursor and return immediately.
///
/// The caller receives the selection as `WM_COMMAND` on `hwnd`.
pub fn show_tray_menu(hwnd: HWND, state: MenuState) -> Result<()> {
    let pause_label = if state.paused {
        w!("Resume monitoring")
    } else {
        w!("Pause monitoring")
    };
    let strip_label = if state.strip_visible {
        w!("Hide taskbar strip")
    } else {
        w!("Show taskbar strip")
    };
    let pause_flags = if state.paused {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING
    };

    // SAFETY: all handles are created and destroyed within this function; the
    // menu is owned by the thread that created it.
    unsafe {
        let menu = CreatePopupMenu()?;
        let _ = AppendMenuW(menu, pause_flags, MENU_PAUSE as usize, pause_label);
        let _ = AppendMenuW(menu, MF_STRING, MENU_TOGGLE_STRIP as usize, strip_label);
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, MENU_DASHBOARD as usize, w!("Dashboard..."));
        let _ = AppendMenuW(menu, MF_STRING, MENU_SETTINGS as usize, w!("Settings..."));
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            MENU_OPEN_DATA as usize,
            w!("Open data folder"),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, MENU_EXIT as usize, w!("Exit"));

        let mut cursor = POINT::default();
        GetCursorPos(&mut cursor)?;

        // Required so the menu dismisses correctly when the user clicks away.
        let _ = SetForegroundWindow(hwnd);
        let shown = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON,
            cursor.x,
            cursor.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        if shown.as_bool() {
            Ok(())
        } else {
            Err(windows::core::Error::from_thread())
        }
    }
}
