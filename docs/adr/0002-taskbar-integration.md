# ADR 0002 — Taskbar integration: our own layered window, three fallback tiers

**Status:** accepted · updated 2026-10-24 after implementation

## Context

Windows 11's XAML taskbar no longer hosts third-party COM deskbands. The remaining
options, with evidence:

| Approach | Verdict |
|---|---|
| `IDeskBand` / `IDeskBand2` | Dead on Win11's XAML taskbar. |
| Inject into Explorer's XAML visual tree (e.g. Windows UI Automation / diagnostics) | Fragile: relies on undocumented element names that change between builds; requires code inside Explorer. Rejected. |
| Docked AppBar (`SHAppBarMessage(ABM_NEW)` + `ABM_SETPOS`) | Documented and stable, but permanently reserves screen space. Too invasive for a default. |
| Tray icon + flyout only | Always works, but not "in the taskbar". Kept as the last fallback. |
| **Own layered window over the taskbar** | What shipping apps do on Win11 (TrafficMonitor, Deskband11). Small, reversible, no injection. |

## Decision

Own layered window, positioned against `TrayNotifyWnd`, in three tiers:

1. **Parented child** of `Shell_TrayWnd` — implemented and the default. Z-order is then
   the shell's problem, so the Start menu and shell surfaces stack correctly.
2. **Topmost popup** over the taskbar — runtime fallback when `SetParent` is refused or
   the first `UpdateLayeredWindowIndirect` on the child fails.
3. **Tray only** — no strip; everything through the tray menu. Selected by `--tray-only`.

## What implementation taught us (supersedes the original plan)

**Tier 1 does work with `UpdateLayeredWindowIndirect`.** The first attempt failed with
`ERROR_GEN_FAILURE (0x8007001F)`, which was blamed on the cross-process parent. The real
cause was the same one that broke the topmost tier: a null `pblend`. With
`BLENDFUNCTION { AC_SRC_OVER, 0, 255, AC_SRC_ALPHA }` supplied, a `WS_CHILD` of
`Shell_TrayWnd` presents correctly. The strip is now parented by default; `adopt` verifies
the parent link actually stuck, and the host detaches to tier 2 at runtime if the first
present on the child fails.

**`UPDATELAYEREDWINDOWINFO.pblend` must not be null** when `ULW_ALPHA` is used. With a
null pointer the call fails with the same misleading `ERROR_GEN_FAILURE`. Do not diagnose
that error code as a driver problem — or as a parenting limitation — without checking
this first.

**Creating the window with another process's window as parent fails** with
`ERROR_CLASS_ALREADY_EXISTS`. Create it standalone as a `WS_POPUP`, set `WS_CHILD`
(keep `WS_VISIBLE`), then `SetParent`.

## Child-window consequences

- Child windows are positioned in the taskbar's **client** coordinates; screen geometry
  is mapped with `ScreenToClient` before every move.
- `TaskbarCreated` is broadcast to top-level windows only. The app's hidden message
  window now receives it and rebuilds the strip for both placement tiers.
- `FindWindow` does not search child windows. Verification and scripts must use
  `FindWindowEx(taskbar, None, "StatLineStripWindow", "StatLine")`.
- `WM_MOUSEACTIVATE` returns `MA_NOACTIVATE`, so clicking the strip never deactivates
  the user's foreground window.
- Hiding the taskbar (fullscreen shell, auto-hide) hides the child with it; the explicit
  fullscreen check in the host stays as a belt-and-braces measure.

## Constraints that follow

- Only long-stable window classes are used: `Shell_TrayWnd`, `TrayNotifyWnd`,
  `Shell_SecondaryTrayWnd`. No XAML element probing.
- Geometry comes from `GetWindowRect` of those windows plus `GetDpiForWindow(taskbar)`,
  never from hardcoded pixel offsets.
- The strip is `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` and layered: never activated, never
  in Alt-Tab, never in the taskbar's own button list.
- In the topmost tier only, z-order is re-asserted on every tick because interacting with
  the taskbar can raise the shell above us. The parented tier does not need this.
- Fullscreen and a hidden taskbar are detected (`SHQueryUserNotificationState`,
  `IsWindowVisible`) and the strip hides rather than guessing.
- On `TaskbarCreated` (Explorer restart) the strip is destroyed and recreated by the app
  message window, which also covers the parented case.

## Consequences

- Works on today's builds without touching Explorer. If a future build renames
  `TrayNotifyWnd`, the anchor falls back to a conservative right-edge band.
- Tier 2 costs one `SetWindowPos(HWND_TOPMOST)` per second; measured cost is noise
  compared with the sampling thread.
