//! Safe wrappers over the Win32 surface Statline needs.
//!
//! This is the only crate (besides `statline-render`'s drawing calls) allowed
//! to contain `unsafe`. Every block carries a `// SAFETY:` justification, and
//! the public API is safe: callers never see raw pointers.

pub mod autostart;
pub mod console;
pub mod dpi;
pub mod logger;
pub mod menus;
pub mod metrics;
pub mod paths;
pub mod shell;
pub mod single_instance;
pub mod strip_window;
pub mod taskbar;
pub mod tray;
pub mod window;

pub use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

/// Convert an `OsStr` to a NUL-terminated UTF-16 buffer for Win32 calls.
pub(crate) fn to_wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

/// Extract the command id from a `WM_COMMAND` message, if this is one.
pub fn command_id(msg: u32, wparam: WPARAM) -> Option<u32> {
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_COMMAND {
        Some((wparam.0 & 0xFFFF) as u32)
    } else {
        None
    }
}

/// `true` when the message is `WM_DESTROY`.
pub fn is_destroy(msg: u32) -> bool {
    msg == windows::Win32::UI::WindowsAndMessaging::WM_DESTROY
}

/// Destroy a window owned by this thread.
pub fn destroy_window(hwnd: HWND) {
    // SAFETY: the caller passes a live window handle created by this thread.
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
    }
}

/// Post `WM_QUIT` to the current thread's message queue.
pub fn quit_message_loop() {
    // SAFETY: no pointers; posts a quit message for this thread.
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn wide_strings_are_nul_terminated() {
        let wide = to_wide(OsStr::new("abc"));
        assert_eq!(wide, vec![b'a' as u16, b'b' as u16, b'c' as u16, 0]);
    }

    #[test]
    fn command_ids_are_extracted_from_the_low_word() {
        assert_eq!(command_id(273, WPARAM(7)), Some(7));
        assert_eq!(command_id(0x0010, WPARAM(7)), None);
    }
}
