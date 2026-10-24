//! Minimal message window and message loop.
//!
//! Statline's main window is invisible: all user-facing surfaces are created
//! on demand (taskbar strip, flyout, dashboard). This window exists to receive
//! tray callbacks and app-wide notifications.

use crate::to_wide;
use std::ffi::OsStr;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG, RegisterClassW,
    TranslateMessage, WINDOW_EX_STYLE, WNDCLASSW, WNDPROC, WS_OVERLAPPED,
};
use windows::core::{PCWSTR, Result};

/// A hidden top-level window used as the app's event sink.
pub struct MessageWindow {
    hwnd: HWND,
}

impl MessageWindow {
    /// Register the window class (idempotent) and create the window.
    pub fn create(class_name: &str, title: &str, wndproc: WNDPROC) -> Result<Self> {
        let class_wide = to_wide(OsStr::new(class_name));
        // SAFETY: converting an opaque module handle; `GetModuleHandleW(None)`
        // returns the current executable's handle.
        let instance = unsafe { GetModuleHandleW(None)? };

        let class = WNDCLASSW {
            lpfnWndProc: wndproc,
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_wide.as_ptr()),
            ..Default::default()
        };

        // SAFETY: `class` is fully initialized and its string pointers remain
        // valid for the duration of the call; the system copies the strings.
        let atom = unsafe { RegisterClassW(&class) };
        if atom == 0 {
            let err = windows::core::Error::from_thread();
            // Re-registering an existing class is fine.
            let code = (err.code().0 as u32) & 0xFFFF;
            if code != 1410 {
                return Err(err);
            }
        }

        let title_wide = to_wide(OsStr::new(title));
        // SAFETY: class name and title are NUL-terminated and outlive the call.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class_wide.as_ptr()),
                PCWSTR(title_wide.as_ptr()),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance.into()),
                None,
            )
        }?;

        Ok(Self { hwnd })
    }

    /// Raw handle, for calls that require one (tray, messages).
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Run the blocking message loop until `WM_QUIT`.
    pub fn run_message_loop() -> Result<i32> {
        let mut msg = MSG::default();
        loop {
            // SAFETY: `msg` is a valid MSG owned by this stack frame.
            let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            match result.0 {
                -1 => return Err(windows::core::Error::from_thread()),
                0 => return Ok(msg.wParam.0 as i32),
                _ => {
                    // SAFETY: standard dispatch of a message retrieved above.
                    unsafe {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
        }
    }
}

/// Default handler for messages the app does not care about.
pub fn default_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: forwarding the exact arguments received by the window procedure.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
