//! The strip window: a non-activating layered window hosted by the taskbar.
//!
//! Tier 1 of the integration strategy in `docs/adr/0002-taskbar-integration.md`:
//! created as an ordinary popup (a cross-process parent at creation fails with
//! `ERROR_CLASS_ALREADY_EXISTS`), then `SetParent`-ed into `Shell_TrayWnd`.
//! Presentation still goes through `UpdateLayeredWindowIndirect`, which works
//! on a layered child since Windows 8. If the shell refuses the parent link, or
//! presentation fails afterwards, the caller falls back to tier 2 (topmost).

use crate::to_wide;
use std::ffi::OsStr;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GWL_STYLE, GetParent, GetWindowLongPtrW, GetWindowRect, HWND_TOPMOST,
    MoveWindow, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetParent, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WNDCLASSW, WNDPROC, WS_CHILD, WS_CLIPSIBLINGS,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{PCWSTR, Result};

/// Class name used by the strip window.
pub const STRIP_CLASS: &str = "StatlineStripWindow";

/// Create the strip window and put it above the taskbar without activating it.
///
/// Always creates a top-level popup; call [`adopt`] afterwards to move it into
/// the taskbar's z-order.
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

/// Adopt the strip as a child of the shell's taskbar (tier 1).
///
/// The window must already exist as a top-level popup. Returns `false` when the
/// shell refuses the parent link; the caller then keeps the topmost placement.
pub fn adopt(hwnd: HWND, taskbar: HWND) -> bool {
    // SAFETY: our window; style bits only, no ownership transfer.
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
    let child_style = (style & !WS_POPUP.0) | WS_CHILD.0 | WS_CLIPSIBLINGS.0;
    // SAFETY: same handle; the new style keeps WS_VISIBLE.
    unsafe {
        SetWindowLongPtrW(hwnd, GWL_STYLE, child_style as isize);
    }

    // SAFETY: changing the parent link; the shell owns `taskbar`, and SetParent
    // does not take ownership of the child.
    if let Err(error) = unsafe { SetParent(hwnd, Some(taskbar)) } {
        log::warn!("could not parent strip into the taskbar: {error}");
        restore_popup_style(hwnd);
        return false;
    }

    // A cross-process parent link can be silently dropped; verify it stuck.
    // SAFETY: plain query on a live window.
    let parent = unsafe { GetParent(hwnd) }.unwrap_or_default();
    if parent != taskbar {
        log::warn!("taskbar did not keep the strip as a child");
        restore_popup_style(hwnd);
        return false;
    }

    // SAFETY: force the frame to recalculate for the new style. The extended
    // WS_EX_TOPMOST bit stays set on a child but is inert there: z-order is
    // relative to the taskbar's children, which is the point of this tier.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
    true
}

/// Undo [`adopt`]: return the strip to a topmost popup.
///
/// The window keeps its position; the caller re-anchors it in screen
/// coordinates on the next tick.
pub fn detach(hwnd: HWND) {
    // SAFETY: our window; `None` re-parents to the desktop.
    unsafe {
        let _ = SetParent(hwnd, None);
    }
    restore_popup_style(hwnd);
    // SAFETY: our window; restore the tier-2 z-order.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

fn restore_popup_style(hwnd: HWND) {
    // SAFETY: our window; style bits only.
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
    let popup_style = (style & !(WS_CHILD.0 | WS_CLIPSIBLINGS.0)) | WS_POPUP.0;
    // SAFETY: same handle; WS_VISIBLE is preserved.
    unsafe {
        SetWindowLongPtrW(hwnd, GWL_STYLE, popup_style as isize);
    }
}

/// Translate a screen rectangle into a parent window's client coordinates.
///
/// Child windows are positioned relative to their parent's client area, so the
/// strip's screen geometry must be mapped before `MoveWindow`.
pub fn parent_client_rect(parent: HWND, rect: RECT) -> RECT {
    let mut top_left = POINT {
        x: rect.left,
        y: rect.top,
    };
    let mut bottom_right = POINT {
        x: rect.right,
        y: rect.bottom,
    };
    // SAFETY: both points are valid out-parameters; a failure leaves them as is.
    unsafe {
        let _ = ScreenToClient(parent, &mut top_left);
        let _ = ScreenToClient(parent, &mut bottom_right);
    }
    RECT {
        left: top_left.x,
        top: top_left.y,
        right: bottom_right.x,
        bottom: bottom_right.y,
    }
}

/// Resize/move a strip window in the coordinates its parent expects.
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

/// Re-assert topmost order for the tier-2 (unparented) strip.
///
/// Called on every tick because clicking the taskbar can raise the shell above
/// us. A parented strip is ordered by the shell and must not use this.
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

/// Create an activatable layered popup (the flyout).
///
/// Unlike the strip this window takes focus, so `WM_ACTIVATE` gives us the
/// natural "the user clicked elsewhere" signal to close it.
pub fn create_popup_window(
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
    // SAFETY: descriptor is fully initialized; a duplicate class name is fine.
    unsafe {
        let _ = RegisterClassW(&class);
    }

    let title_wide = to_wide(OsStr::new(title));
    let width = (rect.right - rect.left).max(1);
    let height = (rect.bottom - rect.top).max(1);

    // SAFETY: strings outlive the call; layered for per-pixel alpha, toolwindow
    // to keep it out of Alt-Tab.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(WS_EX_LAYERED.0 | WS_EX_TOOLWINDOW.0),
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

    // SAFETY: our window; topmost so it sits above the taskbar.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            rect.left,
            rect.top,
            width,
            height,
            SWP_SHOWWINDOW,
        );
        let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
    }
    Ok(hwnd)
}

/// Destroy a window owned by this thread.
///
/// Guards on `IsWindow`: after an Explorer restart the parented strip's handle
/// may already be gone, and destroying a stale handle is not worth the risk.
pub fn destroy(hwnd: HWND) {
    // SAFETY: the handle is ours; the existence check keeps a stale handle from
    // reaching DestroyWindow.
    unsafe {
        if windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(hwnd)).as_bool() {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
        }
    }
}

/// Show or hide the strip without activating it.
pub fn set_visible(hwnd: HWND, visible: bool) {
    // SAFETY: the handle is ours; SW_SHOWNOACTIVATE keeps focus where it was.
    unsafe {
        let _ = ShowWindow(hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
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
