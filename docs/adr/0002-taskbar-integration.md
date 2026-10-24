# ADR 0002 — Taskbar integration: our own child window, three fallback tiers

**Status:** accepted · 2026-10-24

## Context

Windows 11's XAML taskbar no longer hosts third-party COM deskbands. The remaining
options, with evidence:

| Approach | Verdict |
|---|---|
| `IDeskBand` / `IDeskBand2` | Dead on Win11's XAML taskbar. |
| Inject into Explorer's XAML visual tree (e.g. Windows UI Automation / diagnostics) | Fragile: relies on undocumented element names that change between builds; requires code inside Explorer. Rejected. |
| Docked AppBar (`SHAppBarMessage(ABM_NEW)` + `ABM_SETPOS`) | Documented and stable, but permanently reserves screen space. Too invasive for a default. |
| Tray icon + flyout only | Always works, but not "in the taskbar". Kept as the last fallback. |
| **Own layered window + `SetParent(hwnd, Shell_TrayWnd)`** | What shipping apps do on Win11 (TrafficMonitor's `CTaskBarDlg`, Deskband11). Small, reversible, no injection. |

## Decision

Create our own `WS_CHILD | WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` window,
`SetParent` it to the taskbar, and position it against `TrayNotifyWnd`. If parenting
fails or layout drifts, degrade in this order:

1. **Parented strip** — child of `Shell_TrayWnd`, z-order guaranteed.
2. **Topmost strip** — same geometry, re-asserted on `EVENT_SYSTEM_FOREGROUND`.
3. **Tray only** — no strip; everything through the tray flyout.

A user-facing "safe mode" forces tier 2/3, and the Store build can be flagged into it
without a code change.

## Constraints that follow

- Only long-stable window classes are used: `Shell_TrayWnd`, `TrayNotifyWnd`,
  `Shell_SecondaryTrayWnd`. No XAML element probing.
- Geometry is computed from `GetWindowRect` of those windows plus
  `GetDpiForWindow(taskbar)`, never from hardcoded pixel offsets.
- On `TaskbarCreated` (Explorer restart) every shell surface is destroyed and recreated.
- The strip is never activated: `WM_MOUSEACTIVATE` returns `MA_NOACTIVATE`.
- Fullscreen and auto-hide are detected (`SHQueryUserNotificationState`,
  `SHAppBarMessage(ABM_GETSTATE)`) and the strip hides rather than guessing.

## Consequences

- Works on today's builds without touching Explorer. If a future build renames
  `TrayNotifyWnd`, the topmost tier still places the strip correctly, and the tray
  tier always works.
- We own the failure handling: every geometry call is checked and logged, and an
  unsupported layout produces a visible "compatibility mode" state, not a blank strip.
