//! The strip window: a topmost, non-activating layered popup.
//!
//! Tier 2 of the integration strategy in `docs/adr/0002-taskbar-integration.md`.
//! A cross-process child window looks right until presentation:
//! `UpdateLayeredWindowIndirect` fails on it with `ERROR_GEN_FAILURE`, so the
//! strip is a top-level layered window placed over the taskbar instead.
//! Tier 1 (parented child) is revisited in M3 with DirectComposition.

use crate::to_wide;
use std::ffi::OsStr;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetWindowRect, HWND_TOPMOST, MoveWindow, RegisterClassW, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SetWindowPos, WINDOW_EX_STYLE, WNDCLASSW, WNDPROC, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{PCWSTR, Result};

/// Class name used by the strip window.
pub const STRIP_CLASS: &str = "StatlineStripWindow";

/// Create the strip window and put it above the taskbar without activating it.
pub fn create_strip_window(
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
    // SAFETY: the descriptor is fully initialized and its strings live for the
    // duration of the call. A pre-existing class is fine.
    unsafe {
        let _ = RegisterClassW(&class);
    }

    let title_wide = to_wide(OsStr::new(title));
    let width = (rect.right - rect.left).max(1);
    let height = (rect.bottom - rect.top).max(1);

    // SAFETY: strings are NUL-terminated and outlive the call; there is no
    // parent, so this is an ordinary top-level popup.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(WS_EX_LAYERED.0 | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0),
            PCWSTR(class_wide.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            WS_POPUP,
            rect.left,
            rect.top,
            width,
            height,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }?;

    // SAFETY: the handle belongs to this thread; the call only changes
    // z-order and visibility, and must not activate the window.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            rect.left,
            rect.top,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
    Ok(hwnd)
}

/// Resize/move a strip window (screen coordinates).
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

/// Re-assert topmost order. Called on every tick because clicking the taskbar
/// can raise the shell above us.
pub fn keep_on_top(hwnd: HWND) {
    // SAFETY: the handle is ours; no activation and no geometry change.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE
                | SWP_NOACTIVATE,
        );
    }
}

/// Screen rectangle of a window.
pub fn window_rect(hwnd: HWND) -> RECT {
    let mut rect = RECT::default();
    // SAFETY: valid out-parameter for a live window.
    unsafe {
        let _ = GetWindowRect(hwnd, &mut rect);
    }
    rect
}
