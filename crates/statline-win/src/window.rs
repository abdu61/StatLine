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

/// Options for a framed window.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameOptions {
    pub width: i32,
    pub height: i32,
    pub dark: bool,
    pub mica: bool,
    /// Remembered outer rectangle (x, y, width, height) in physical pixels.
    pub bounds: Option<(i32, i32, i32, i32)>,
}

/// Create an ordinary framed window: caption, system menu, minimise and close.
///
/// The companion and settings windows are normal Windows apps, so they get a
/// normal frame. The Windows 11 treatment is applied explicitly: immersive dark
/// mode when `dark` is set, rounded corners, and an optional Mica backdrop.
pub fn create_frame_window(
    class_name: &str,
    title: &str,
    wndproc: WNDPROC,
    options: FrameOptions,
) -> Result<HWND> {
    let FrameOptions {
        width,
        height,
        dark,
        mica,
        bounds,
    } = options;
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
        hIcon: crate::tray::app_icon().unwrap_or_default(),
        ..Default::default()
    };
    unsafe {
        let _ = RegisterClassW(&class);
    }

    let title_wide = to_wide(OsStr::new(title));
    let (x, y, width, height) = match bounds {
        Some((x, y, width, height)) => (x, y, width, height),
        None => (
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            width,
            height,
        ),
    };
    // SAFETY: strings outlive the call; a normal overlapped window with a
    // caption and a system menu.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_wide.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }?;

    apply_frame_style(hwnd, dark, mica);

    // SAFETY: our window; show it normally.
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
    Ok(hwnd)
}

/// Whether any part of `rect` intersects a monitor.
///
/// Used before restoring a remembered window position; a monitor that was
/// disconnected since the last run must not strand the window off-screen.
pub fn rect_is_visible(rect: (i32, i32, i32, i32)) -> bool {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromRect};

    let (x, y, width, height) = rect;
    let rect = RECT {
        left: x,
        top: y,
        right: x.saturating_add(width),
        bottom: y.saturating_add(height),
    };
    // SAFETY: `rect` is a live, fully initialized RECT.
    let monitor = unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONULL) };
    !monitor.0.is_null()
}

fn apply_frame_style(hwnd: HWND, dark: bool, mica: bool) {
    use windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};

    // DWMWA_USE_IMMERSIVE_DARK_MODE = 20, DWMWA_WINDOW_CORNER_PREFERENCE = 33.
    let dark = i32::from(dark);
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
    set_backdrop(hwnd, mica);
}

/// Turn the Windows 11 Mica system backdrop on or off for a framed window.
///
/// `DWMWA_SYSTEMBACKDROP_TYPE` is attribute 38; 2 is Mica, 1 is none. The call
/// is a no-op on Windows 10 and on machines where the compositor disables it.
pub fn set_backdrop(hwnd: HWND, mica: bool) {
    use windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};

    let value: i32 = if mica { 2 } else { 1 };
    // SAFETY: attribute write only; the value is a valid i32 for this attribute.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(38),
            (&value as *const i32).cast(),
            std::mem::size_of::<i32>() as u32,
        );
    }
}
