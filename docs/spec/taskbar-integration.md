# Spec — taskbar integration

## Finding the taskbar

```text
primary:    FindWindowW("Shell_TrayWnd", null)
secondary:  EnumWindows → class "Shell_SecondaryTrayWnd" (one per monitor with a taskbar)
tray:       FindWindowExW(taskbar, null, "TrayNotifyWnd", null)
start:      FindWindowExW(taskbar, null, "Start", null)
dpi:        GetDpiForWindow(taskbar)
```

`MSTaskSwWClass` no longer exists on Windows 11's XAML taskbar. Do not depend on it.

## Geometry (in physical pixels; the process is per-monitor-v2 aware)

```
taskbar      = GetWindowRect(taskbar)
tray         = GetWindowRect(TrayNotifyWnd)
strip.height = taskbar.height - 2 * pad      (pad = 4 dip)
strip.top    = taskbar.top + (taskbar.height - strip.height) / 2
placement "right" (default):
    strip.right = tray.left - 2
    strip.left  = strip.right - measured_width
placement "left":
    strip.left  = start.right + 4  (falls back to taskbar.left + 4)
```

Guard rails:

- Never overlap `TrayNotifyWnd` or `Start`; if `strip.left < start.right + 4`, shrink
  segments (drop text, keep meters), then give up and use the tray-only tier.
- Recompute on every tick (cheap: three `GetWindowRect` calls) and on
  `WM_DISPLAYCHANGE`, `WM_DPICHANGED`, `WM_SETTINGCHANGE`, `TaskbarCreated`.
- Vertical taskbars (if a future build restores them): hide the strip, log once, use
  tray-only. Not a v1 target.

## Window setup

```text
style:      WS_CHILD | WS_CLIPSIBLINGS
ex style:   WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW
parent:     SetParent(hwnd, taskbar)     → failure ⇒ tier 2
present:    UpdateLayeredWindowIndirect  (premultiplied BGRA)
```

- `WM_MOUSEACTIVATE` → `MA_NOACTIVATE`: clicking the strip must not deactivate the
  user's foreground window.
- `WM_NCHITTEST` → `HTCLIENT` only inside the strip's own rect (the window is exactly
  the content size, so nothing to exclude).
- `WM_RBUTTONUP` → tray context menu at the cursor.
- `WM_LBUTTONUP` → toggle the flyout, anchored above the clicked segment.

## Visibility rules

| Condition | Action |
|---|---|
| Fullscreen app / presentation mode (`SHQueryUserNotificationState` = `QUNS_BUSY`, `QUNS_RUNNING_D3D_FULL_SCREEN`, `QUNS_PRESENTATION_MODE`) | hide |
| Taskbar auto-hidden (`ABM_GETSTATE & ABS_AUTOHIDE`) and pointer away from the edge | hide (taskbar rect leaves the monitor) |
| Monitoring paused from the tray menu | hide |
| Explorer restart | destroy + recreate everything |
| Safe mode / unsupported layout | tier 2 (topmost) or tier 3 (tray only) |

## Z-order fallbacks

1. **Parented** — child of `Shell_TrayWnd`. Preferred.
2. **Topmost** — `SetWindowPos(HWND_TOPMOST)` plus re-assertion on
   `EVENT_SYSTEM_FOREGROUND` (`SetWinEventHook`, out-of-context, no injection).
3. **Tray only** — no strip window at all; the flyout opens from the tray icon.

The active tier is visible in Settings → About and in the log at startup.

## Things that broke in other implementations (avoid)

- Hardcoded pixel offsets → wrong at any DPI other than 100 %.
- `SetParent` without handling Explorer restart → orphaned invisible window.
- Assuming a centered taskbar → strip overlaps app icons on left-aligned layouts.
- Continuous animation over the taskbar → measurable battery drain.

## Verifying the live strip

The strip is a child of `Shell_TrayWnd`, so `FindWindow` (top-level only) will not find
it. Use `FindWindowEx` and pass the real title; an empty title matches only windows
whose title is empty:

```powershell
$taskbar = FindWindow("Shell_TrayWnd", $null)
$strip   = FindWindowEx($taskbar, 0, "StatlineStripWindow", "Statline")
GetParent($strip) -eq $taskbar          # must be true
GetWindowLongPtr($strip, GWL_STYLE)     # WS_CHILD | WS_CLIPSIBLINGS, no WS_POPUP
GetWindowLongPtr($strip, GWL_EXSTYLE)   # WS_EX_LAYERED | NOACTIVATE | TOOLWINDOW
```

Position in the log is in physical pixels; a DPI-unaware PowerShell session reports
virtualized coordinates, so do not treat the two as a mismatch.
