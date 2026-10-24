# M3 — taskbar strip

**Status:** not started

## Goal

The product: a live strip living in the taskbar, robust to DPI, theme and shell events.

## Checklist

- [ ] `Shell_TrayWnd` / `TrayNotifyWnd` / `Start` discovery, DPI from the taskbar
- [ ] Child layered window (`WS_EX_LAYERED | NOACTIVATE | TOOLWINDOW`), `SetParent`
- [ ] `UpdateLayeredWindowIndirect` present path, dirty-only redraw
- [ ] Geometry per spec: right/left placement, vertical centring, overlap guards
- [ ] Fallback tier 2 (topmost + `EVENT_SYSTEM_FOREGROUND`) and tier 3 (tray only)
- [ ] `TaskbarCreated` → recreate; `WM_DISPLAYCHANGE`/`WM_DPICHANGED` → relayout
- [ ] Auto-hide + fullscreen hiding (`SHQueryUserNotificationState`)
- [ ] Hover, click, right-click; `WM_MOUSEACTIVATE` → `MA_NOACTIVATE`
- [ ] Live capture verified with `scripts/capture-window.ps1`

## Exit criteria

Strip sits flush between app icons and tray at 125 % on this machine, survives Explorer
restart/DPI change/auto-hide, hides for fullscreen, never steals focus, stays inside
the idle budget for 5 minutes.
