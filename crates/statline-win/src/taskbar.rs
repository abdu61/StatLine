//! Finding and measuring the Windows 11 taskbar.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{
    QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GetWindowRect, IsWindowVisible,
};
use windows::core::w;

/// Everything the strip needs to place itself.
#[derive(Debug, Clone, Copy)]
pub struct TaskbarGeometry {
    pub taskbar: HWND,
    /// Full taskbar rectangle in physical pixels.
    pub rect: RECT,
    /// Left edge of the notification area, in physical pixels.
    pub tray_left: i32,
    pub dpi: u32,
}

impl TaskbarGeometry {
    pub fn height(&self) -> i32 {
        self.rect.bottom - self.rect.top
    }

    /// Where the strip should sit for a given width and padding.
    ///
    /// Anchoring to the tray keeps the strip clear of the app buttons on both
    /// centred and left-aligned taskbars.
    pub fn strip_left(&self, width: i32, padding: i32) -> i32 {
        (self.tray_left - width - padding).max(self.rect.left + padding)
    }
}

/// Measure the primary taskbar, or `None` when the shell is not available.
pub fn primary() -> Option<TaskbarGeometry> {
    // SAFETY: plain window lookups; no pointers leave this function.
    unsafe {
        let taskbar = FindWindowW(w!("Shell_TrayWnd"), None).ok()?;
        if taskbar.is_invalid() || !IsWindowVisible(taskbar).as_bool() {
            return None;
        }

        let mut rect = RECT::default();
        GetWindowRect(taskbar, &mut rect).ok()?;

        let tray =
            FindWindowExW(Some(taskbar), None, w!("TrayNotifyWnd"), None).unwrap_or_default();
        let mut tray_rect = RECT::default();
        if !tray.is_invalid() {
            let _ = GetWindowRect(tray, &mut tray_rect);
        }

        // If the tray cannot be measured (some secondary taskbars), reserve a
        // conservative band at the right edge instead of guessing zero.
        let tray_left = if tray_rect.left > rect.left {
            tray_rect.left
        } else {
            rect.right - (rect.bottom - rect.top) * 6
        };

        let dpi = GetDpiForWindow(taskbar);
        Some(TaskbarGeometry {
            taskbar,
            rect,
            tray_left,
            dpi: if dpi == 0 { 96 } else { dpi },
        })
    }
}

/// True when the shell is currently animating the taskbar out of view.
pub fn is_hidden(taskbar: HWND) -> bool {
    // SAFETY: a window handle from `primary()`; visibility is a cheap query.
    unsafe { !IsWindowVisible(taskbar).as_bool() }
}

/// True when a fullscreen app, video or presentation owns the screen.
///
/// `SHQueryUserNotificationState` is the documented, cheap way to ask; Windows
/// itself uses it to suppress notifications during games and videos.
pub fn fullscreen_active() -> bool {
    // SAFETY: no arguments, no pointers.
    match unsafe { SHQueryUserNotificationState() } {
        Ok(state) => {
            state == QUNS_BUSY
                || state == QUNS_RUNNING_D3D_FULL_SCREEN
                || state == QUNS_PRESENTATION_MODE
        }
        Err(_) => false,
    }
}
