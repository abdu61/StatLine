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

1. **Parented child** of `Shell_TrayWnd` — preferred in principle, because z-order is
   then the shell's problem.
2. **Topmost popup** over the taskbar, re-asserted every tick — current default.
3. **Tray only** — no strip; everything through the tray menu. Selected by `--tray-only`.

## What implementation taught us (supersedes the original plan)

**Tier 1 does not work with `UpdateLayeredWindowIndirect`.** A window created as a child
of the cross-process taskbar, presented with ULW, fails with
`ERROR_GEN_FAILURE (0x8007001F)` — irrespective of D2D (a raw DIB fill fails the same
way). Tier 2 presents correctly. DirectComposition on a child HWND remains a possible
route back to tier 1 and is not required for the strip to work.

**`UPDATELAYEREDWINDOWINFO.pblend` must not be null** when `ULW_ALPHA` is used. With a
null pointer the call fails with the same misleading `ERROR_GEN_FAILURE`. Passing
`BLENDFUNCTION { AC_SRC_OVER, 0, 255, AC_SRC_ALPHA }` is what made presentation work.
Do not diagnose that error code as a driver problem without checking this first.

**Creating the window with another process's window as parent fails** with
`ERROR_CLASS_ALREADY_EXISTS`. Create it standalone, then `SetParent`.

## Constraints that follow

- Only long-stable window classes are used: `Shell_TrayWnd`, `TrayNotifyWnd`,
  `Shell_SecondaryTrayWnd`. No XAML element probing.
- Geometry comes from `GetWindowRect` of those windows plus `GetDpiForWindow(taskbar)`,
  never from hardcoded pixel offsets.
- The strip is a `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` popup: never activated, never in
  Alt-Tab, never in the taskbar's own button list.
- Topmost is re-asserted on every tick because interacting with the taskbar can raise
  the shell above us.
- Fullscreen and a hidden taskbar are detected (`SHQueryUserNotificationState`,
  `IsWindowVisible`) and the strip hides rather than guessing.
- On `TaskbarCreated` (Explorer restart) the strip must be destroyed and recreated —
  still on the M3 checklist.

## Consequences

- Works on today's builds without touching Explorer. If a future build renames
  `TrayNotifyWnd`, the anchor falls back to a conservative right-edge band.
- Tier 2 costs one `SetWindowPos(HWND_TOPMOST)` per second; measured cost is noise
  compared with the sampling thread.
