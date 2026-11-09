//! Notification-area (tray) icon.
//!
//! Callback protocol uses `NOTIFYICON_VERSION_4`, so the callback message's
//! `lParam` carries the event id and `wParam` carries packed cursor coordinates.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NIM_SETVERSION, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HICON, IDI_APPLICATION, LoadIconW, WM_APP, WM_CONTEXTMENU, WM_LBUTTONUP, WM_RBUTTONUP,
};
use windows::core::{PCWSTR, Result};

/// Message posted to the owner window when the user interacts with the icon.
pub const WM_TRAY: u32 = WM_APP + 1;

/// `NIN_SELECT` (equals `WM_USER`): sent on click for version-4 icons.
pub const NIN_SELECT: u32 = 0x0400;
/// `NIN_KEYSELECT`: sent when the icon is activated from the keyboard.
pub const NIN_KEYSELECT: u32 = 0x0401;

/// Events we care about, decoded from the callback message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    /// Primary click (mouse or keyboard).
    Activate,
    /// Context menu request.
    ContextMenu,
}

impl TrayEvent {
    /// Decode a version-4 callback `lParam`.
    pub fn from_callback(lparam: LPARAM) -> Option<Self> {
        match lparam.0 as u32 {
            WM_LBUTTONUP | NIN_SELECT | NIN_KEYSELECT => Some(Self::Activate),
            WM_RBUTTONUP | WM_CONTEXTMENU => Some(Self::ContextMenu),
            _ => None,
        }
    }
}

fn icon_data(hwnd: HWND, id: u32) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: id,
        ..Default::default()
    };
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
    data.uCallbackMessage = WM_TRAY;
    data
}

/// Add the tray icon. Uses the embedded app icon when the build included one,
/// otherwise the stock application icon.
pub fn add(hwnd: HWND, id: u32, tooltip: &str) -> Result<()> {
    // Resource id 1 is the icon the build script embeds. The call fails when a
    // build skipped the SDK, so fall back to the system icon.
    let icon: HICON = app_icon()
        .or_else(|| {
            // SAFETY: `IDI_APPLICATION` is a predefined resource identifier, so no
            // module handle is required. The returned HICON is owned by the system
            // and must not be destroyed.
            unsafe { LoadIconW(None, IDI_APPLICATION) }.ok()
        })
        .ok_or_else(windows::core::Error::from_thread)?;

    let mut data = icon_data(hwnd, id);
    data.hIcon = icon;
    copy_wide(tooltip, &mut data.szTip);

    // SAFETY: `data` is fully initialized and sized via `cbSize`.
    let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
    if !added.as_bool() {
        return Err(windows::core::Error::from_thread());
    }

    let mut version = icon_data(hwnd, id);
    version.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    // SAFETY: same contract as above; NIM_SETVERSION only reads `uVersion`.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_SETVERSION, &version);
    }
    Ok(())
}

/// Remove the tray icon. Safe to call when the icon is not present.
pub fn remove(hwnd: HWND, id: u32) {
    let data = icon_data(hwnd, id);
    // SAFETY: `data` is a valid, correctly sized NOTIFYICONDATAW.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

/// The embedded resource icon, if this build had one.
pub fn app_icon() -> Option<HICON> {
    // SAFETY: `GetModuleHandleW(None)` is the current executable. The pointer is
    // MAKEINTRESOURCE(1), the integer id used by the build script, not a string.
    let module = unsafe { GetModuleHandleW(None) }.ok()?;
    unsafe { LoadIconW(Some(module.into()), PCWSTR(std::ptr::without_provenance(1))) }.ok()
}

/// Update the icon's hover tooltip.
///
/// `NOTIFYICONDATAW.szTip` is 128 characters; longer text is truncated rather
/// than rejected. Callers should only call this when the text changed, since it
/// crosses into the shell on every update.
pub fn set_tooltip(hwnd: HWND, id: u32, tooltip: &str) -> Result<()> {
    let mut data = icon_data(hwnd, id);
    data.uFlags = NIF_TIP;
    copy_wide(tooltip, &mut data.szTip);

    // SAFETY: `data` is fully initialized and sized via `cbSize`; the icon
    // already exists, so NIM_MODIFY only updates the tooltip.
    let updated = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
    if updated.as_bool() {
        Ok(())
    } else {
        Err(windows::core::Error::from_thread())
    }
}

/// Show a balloon notification from the existing tray icon.
///
/// Used once on first run to state the access posture; Focus Assist may suppress
/// it, which is why the same facts live in About and `docs/ACCESS.md`.
pub fn notify(hwnd: HWND, id: u32, title: &str, message: &str) -> Result<()> {
    let mut data = icon_data(hwnd, id);
    data.uFlags = NIF_INFO;
    data.dwInfoFlags = NIIF_INFO;
    copy_wide(title, &mut data.szInfoTitle);
    copy_wide(message, &mut data.szInfo);

    // SAFETY: `data` is fully initialized and sized via `cbSize`; the icon
    // already exists, so NIM_MODIFY only adds the balloon fields.
    let shown = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
    if shown.as_bool() {
        Ok(())
    } else {
        Err(windows::core::Error::from_thread())
    }
}

fn copy_wide(value: &str, target: &mut [u16]) {
    let wide: Vec<u16> = OsStr::new(value)
        .encode_wide()
        .take(target.len().saturating_sub(1))
        .collect();
    let len = wide.len();
    target[..len].copy_from_slice(&wide);
    target[len] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_click_events() {
        assert_eq!(
            TrayEvent::from_callback(LPARAM(WM_LBUTTONUP as isize)),
            Some(TrayEvent::Activate)
        );
        assert_eq!(
            TrayEvent::from_callback(LPARAM(WM_CONTEXTMENU as isize)),
            Some(TrayEvent::ContextMenu)
        );
        assert_eq!(TrayEvent::from_callback(LPARAM(0x1234)), None);
    }

    #[test]
    fn tooltip_is_truncated_and_terminated() {
        let mut buf = [0u16; 8];
        copy_wide("StatLine monitor", &mut buf);
        assert_eq!(buf[7], 0);
        assert_eq!(buf[0], b'S' as u16);
    }
}
