//! Minimal message window and message loop.
//!
//! Statline's main window is invisible: all user-facing surfaces are created
//! on demand (taskbar strip, flyout, dashboard). This window exists to receive
//! tray callbacks and app-wide notifications.

use crate::to_wide;
use std::ffi::OsStr;
use std::sync::OnceLock;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG, RegisterClassW,
    RegisterWindowMessageW, TranslateMessage, WINDOW_EX_STYLE, WNDCLASSW, WNDPROC, WS_OVERLAPPED,
};
use windows::core::{PCWSTR, Result, w};

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

static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();

/// The broadcast message Explorer sends after (re)starting.
///
/// Cached because the id is stable for the process lifetime and this is compared
/// against every dispatched message.
pub fn taskbar_created_message() -> u32 {
    *TASKBAR_CREATED.get_or_init(|| {
        // SAFETY: registering a message name; no pointers leave the call.
        unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) }
    })
}

/// Create an ordinary framed window: caption, system menu, minimise and close.
///
/// The companion app is a normal Windows app, so it gets a normal frame. The
/// Windows 11 treatment is applied explicitly: immersive dark mode when the
/// system theme is dark, and rounded corners.
pub fn create_frame_window(
    class_name: &str,
    title: &str,
    wndproc: WNDPROC,
    width: i32,
    height: i32,
) -> Result<HWND> {
    let class_wide = to_wide(OsStr::new(class_name));
    let instance = unsafe { GetModuleHandleW(None)? };

    let hbr_background = unsafe {
        windows::Win32::Graphics::Gdi::CreateSolidBrush(windows::Win32::Foundation::COLORREF(
            0x0020_2020,
        ))
    };

    let class = WNDCLASSW {
        lpfnWndProc: wndproc,
        hInstance: instance.into(),
        lpszClassName: PCWSTR(class_wide.as_ptr()),
        hbrBackground: hbr_background,
        ..Default::default()
    };
    unsafe {
        let _ = RegisterClassW(&class);
    }

    let title_wide = to_wide(OsStr::new(title));
    // SAFETY: strings outlive the call; a normal overlapped window with a
    // caption and a system menu.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_wide.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            width,
            height,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }?;

    apply_frame_style(hwnd);

    // SAFETY: our window; show it normally.
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
    Ok(hwnd)
}

fn apply_frame_style(hwnd: HWND) {
    use windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};

    // DWMWA_USE_IMMERSIVE_DARK_MODE = 20, DWMWA_WINDOW_CORNER_PREFERENCE = 33.
    let dark: i32 = 1;
    let round: u32 = 2; // DWMWCP_ROUND
    // SAFETY: attribute writes only; unsupported attributes fail harmlessly on
    // older builds, which is why the results are ignored.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(20),
            (&dark as *const i32).cast(),
            std::mem::size_of::<i32>() as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(33),
            (&round as *const u32).cast(),
            std::mem::size_of::<u32>() as u32,
        );
    }
}
