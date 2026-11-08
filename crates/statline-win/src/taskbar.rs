//! Finding and measuring the Windows 11 taskbar.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{
    QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GetWindowRect, IsWindowVisible,
};
use windows::core::{PCWSTR, w};

/// Which side of the taskbar the strip sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placement {
    #[default]
    Right,
    Left,
}

/// Everything the strip needs to place itself.
#[derive(Debug, Clone, Copy)]
pub struct TaskbarGeometry {
    pub taskbar: HWND,
    /// Full taskbar rectangle in physical pixels.
    pub rect: RECT,
    /// Left edge of the notification area, in physical pixels.
    pub tray_left: i32,
    /// Right edge of the Start button, when the shell exposes it.
    pub start_right: Option<i32>,
    /// Edges of the application-button group (`ReBarWindow32`), when present.
    pub app_left: Option<i32>,
    pub app_right: Option<i32>,
    /// Windows 11 taskbar alignment. `true` means centred.
    pub centered: bool,
    pub dpi: u32,
}

impl TaskbarGeometry {
    pub fn height(&self) -> i32 {
        self.rect.bottom - self.rect.top
    }

    /// Where the strip should sit for a given width, padding and placement.
    ///
    /// Right (default) anchors to the tray, which keeps the strip clear of the
    /// app buttons on both centred and left-aligned taskbars.
    ///
    /// Left means "leftmost free space":
    /// - centred taskbar: the free area before the centred group (the far left);
    /// - left-aligned taskbar: just after the app-button group, because Start and
    ///   the app buttons occupy the left edge.
    pub fn strip_left_for(&self, width: i32, padding: i32, placement: Placement) -> i32 {
        let right_edge = (self.tray_left - width - padding).max(self.rect.left + padding);
        match placement {
            Placement::Right => right_edge,
            Placement::Left if self.centered => {
                let edge = self.rect.left + padding;
                // Keep the strip clear of the centred group when it fits; a
                // very wide strip cannot do better than the left edge.
                match self.app_left {
                    Some(app_left) => edge.min((app_left - width - padding).max(self.rect.left)),
                    None => edge,
                }
            }
            Placement::Left => {
                // `ReBarWindow32` covers the app buttons; Start is inside the
                // same visual group on Windows 11 but is not a measurable child.
                let occupied_right = self
                    .start_right
                    .into_iter()
                    .chain(self.app_right)
                    .max()
                    .unwrap_or(self.rect.left);
                (occupied_right + padding).min(right_edge)
            }
        }
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

        let start = FindWindowExW(Some(taskbar), None, w!("Start"), None).unwrap_or_default();
        let mut start_rect = RECT::default();
        let start_right = if !start.is_invalid() && GetWindowRect(start, &mut start_rect).is_ok() {
            (start_rect.right > rect.left).then_some(start_rect.right)
        } else {
            None
        };

        let rebar =
            FindWindowExW(Some(taskbar), None, w!("ReBarWindow32"), None).unwrap_or_default();
        let mut rebar_rect = RECT::default();
        let (app_left, app_right) =
            if !rebar.is_invalid() && GetWindowRect(rebar, &mut rebar_rect).is_ok() {
                (
                    Some(rebar_rect.left).filter(|left| *left >= rect.left),
                    Some(rebar_rect.right).filter(|right| *right <= rect.right),
                )
            } else {
                (None, None)
            };

        let centered = taskbar_centered().unwrap_or_else(|| {
            // Fallback: a group whose centre is near the taskbar centre is the
            // Windows 11 centred layout.
            match (app_left, app_right) {
                (Some(left), Some(right)) => {
                    let group = (left + right) / 2;
                    let middle = (rect.left + rect.right) / 2;
                    (group - middle).abs() < (rect.right - rect.left) / 5
                }
                _ => false,
            }
        });

        let dpi = GetDpiForWindow(taskbar);
        Some(TaskbarGeometry {
            taskbar,
            rect,
            tray_left,
            start_right,
            app_left,
            app_right,
            centered,
            dpi: if dpi == 0 { 96 } else { dpi },
        })
    }
}

/// Windows 11 taskbar alignment from `TaskbarAl` (0 = left, 1 = centre).
///
/// `None` on builds that do not expose the value; callers then infer from the
/// application group's position.
pub fn taskbar_centered() -> Option<bool> {
    const ADVANCED: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced");
    const TASKBAR_AL: PCWSTR = w!("TaskbarAl");

    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: static key/value names; `value` and `size` are valid DWORD
    // out-parameters.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            ADVANCED,
            TASKBAR_AL,
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    status.is_ok().then_some(value != 0)
}

/// The primary taskbar window handle, without the geometry walk.
///
/// `primary()` reads the registry and measures several children, so callers that
/// only need the handle (a cheap `GetWindowRect` to notice a move) should use
/// this instead.
pub fn taskbar_window() -> Option<HWND> {
    // SAFETY: plain window lookup; no pointers escape.
    let taskbar = unsafe { FindWindowW(w!("Shell_TrayWnd"), None) }.ok()?;
    (!taskbar.is_invalid()).then_some(taskbar)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> TaskbarGeometry {
        TaskbarGeometry {
            taskbar: HWND::default(),
            rect: RECT {
                left: 0,
                top: 1000,
                right: 1920,
                bottom: 1080,
            },
            tray_left: 1700,
            start_right: Some(40),
            app_left: Some(0),
            app_right: Some(40),
            centered: false,
            dpi: 96,
        }
    }

    #[test]
    fn placement_anchors_to_the_expected_edge() {
        let geometry = geometry();
        assert_eq!(geometry.strip_left_for(200, 4, Placement::Right), 1496);
        assert_eq!(geometry.strip_left_for(200, 4, Placement::Left), 44);
    }

    #[test]
    fn left_placement_never_overruns_the_tray() {
        let geometry = TaskbarGeometry {
            start_right: Some(1900),
            ..geometry()
        };
        assert_eq!(geometry.strip_left_for(200, 4, Placement::Left), 1496);
    }

    #[test]
    fn missing_shell_rects_fall_back_to_the_taskbar_edge() {
        let geometry = TaskbarGeometry {
            start_right: None,
            app_left: None,
            app_right: None,
            ..geometry()
        };
        assert_eq!(geometry.strip_left_for(200, 4, Placement::Left), 4);
    }

    #[test]
    fn left_aligned_taskbar_anchors_after_the_app_group() {
        let geometry = TaskbarGeometry {
            start_right: Some(70),
            app_left: Some(55),
            app_right: Some(407),
            centered: false,
            ..geometry()
        };
        assert_eq!(geometry.strip_left_for(200, 4, Placement::Left), 411);
    }

    #[test]
    fn centered_taskbar_uses_the_free_left_edge() {
        let geometry = TaskbarGeometry {
            start_right: Some(700),
            app_left: Some(620),
            app_right: Some(980),
            centered: true,
            ..geometry()
        };
        assert_eq!(geometry.strip_left_for(200, 4, Placement::Left), 4);
    }

    #[test]
    fn alignment_query_does_not_panic() {
        let _ = taskbar_centered();
    }
}
