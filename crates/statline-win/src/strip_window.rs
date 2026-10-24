//! Create a layered child window parented to the taskbar.

use crate::to_wide;
use std::ffi::OsStr;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetWindowRect, MoveWindow, RegisterClassW, SW_SHOWNOACTIVATE, ShowWindow,
    WINDOW_EX_STYLE, WNDCLASSW, WNDPROC, WS_CHILD, WS_CLIPSIBLINGS,
};
use windows::core::{PCWSTR, Result, w};

/// Class name used by the strip window.
pub const STRIP_CLASS: &str = "StatlineStripWindow";

/// Create (or reuse) a layered child window of `parent` and show it without
/// activating it.
pub fn create_layered_child(
    parent: HWND,
    class_name: &str,
    title: &str,
    wndproc: WNDPROC,
    rect: RECT,
) -> Result<HWND> {
    let class_wide = to_wide(OsStr::new(class_name));
    // SAFETY: converting an opaque module handle for the current executable.
    let instance = unsafe { GetModuleHandleW(None)? };

    let class = WNDCLASSW {
        lpfnWndProc: wndproc,
        hInstance: instance.into(),
        lpszClassName: PCWSTR(class_wide.as_ptr()),
        ..Default::default()
    };
    // SAFETY: the class descriptor is fully initialized and the strings live
    // for the duration of the call.
    let atom = unsafe { RegisterClassW(&class) };
    if atom == 0 {
        // A pre-existing class is fine; anything else will fail below.
        let _ = windows::core::Error::from_thread();
    }

    let title_wide = to_wide(OsStr::new(title));
    // SAFETY: strings are NUL-terminated and outlive the call; the parent is
    // the shell's taskbar window.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(
                windows::Win32::UI::WindowsAndMessaging::WS_EX_LAYERED.0
                    | windows::Win32::UI::WindowsAndMessaging::WS_EX_NOACTIVATE.0
                    | windows::Win32::UI::WindowsAndMessaging::WS_EX_TOOLWINDOW.0,
            ),
            PCWSTR(class_wide.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::WS_POPUP | WS_CLIPSIBLINGS,
            rect.left,
            rect.top,
            (rect.right - rect.left).max(1),
            (rect.bottom - rect.top).max(1),
            None,
            None,
            Some(instance.into()),
            None,
        )
    }?;

    // Cross-process parenting is only reliable after creation.
    // SAFETY: both handles are live; SetParent changes only the parent link.
    unsafe {
        // A child must carry WS_CHILD; SetParent alone leaves it a popup.
        let style = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::GWL_STYLE,
        );
        let child = (style as u32 | windows::Win32::UI::WindowsAndMessaging::WS_CHILD.0)
            & !windows::Win32::UI::WindowsAndMessaging::WS_POPUP.0;
        let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::GWL_STYLE,
            child as isize,
        );
        let previous = windows::Win32::UI::WindowsAndMessaging::SetParent(hwnd, Some(parent));
        if let Err(error) = previous {
            log::warn!("could not parent strip to taskbar: {error}");
        }
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    Ok(hwnd)
}

/// Resize/move a strip window.
pub fn move_window(hwnd: HWND, rect: RECT) {
    // SAFETY: the handle is ours; repaint is delegated to the layered surface.
    unsafe {
        let _ = MoveWindow(
            hwnd,
            rect.left,
            rect.top,
            (rect.right - rect.left).max(1),
            (rect.bottom - rect.top).max(1),
            true,
        );
    }
}

/// Client-independent window rect.
pub fn window_rect(hwnd: HWND) -> RECT {
    let mut rect = RECT::default();
    // SAFETY: valid out-parameter for a live window.
    unsafe {
        let _ = GetWindowRect(hwnd, &mut rect);
    }
    rect
}

/// Ensure the class used by the strip exists before creating the window.
pub fn register_strip_class(wndproc: WNDPROC) -> Result<()> {
    let class_wide = to_wide(OsStr::new(STRIP_CLASS));
    // SAFETY: module handle conversion only.
    let instance = unsafe { GetModuleHandleW(None)? };
    let class = WNDCLASSW {
        lpfnWndProc: wndproc,
        hInstance: instance.into(),
        lpszClassName: PCWSTR(class_wide.as_ptr()),
        ..Default::default()
    };
    // SAFETY: descriptor is valid for the call.
    let atom = unsafe { RegisterClassW(&class) };
    if atom == 0 {
        let error = windows::core::Error::from_thread();
        if (error.code().0 as u32) & 0xFFFF != 1410 {
            return Err(error);
        }
    }
    let _ = w!("strip");
    Ok(())
}
