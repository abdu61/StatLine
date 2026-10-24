//! Context menus.

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_SEPARATOR, MF_STRING, PostMessageW,
    SetForegroundWindow, TPM_RIGHTBUTTON, TrackPopupMenu, WM_NULL,
};
use windows::core::{PCWSTR, Result, w};

/// Open the folder that holds settings and logs.
pub const MENU_OPEN_DATA: u32 = 1;
/// Quit the application.
pub const MENU_EXIT: u32 = 2;

/// Show the tray context menu at the cursor and return immediately.
///
/// The caller receives the selection as `WM_COMMAND` on `hwnd`.
pub fn show_tray_menu(hwnd: HWND) -> Result<()> {
    // SAFETY: all handles are created and destroyed within this function; the
    // menu is owned by the thread that created it.
    unsafe {
        let menu = CreatePopupMenu()?;
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
