# M4 — tray, lifecycle, settings I/O

**Status:** not started

## Goal

A well-behaved Windows citizen: one instance, correct startup behaviour, real menu,
recoverable failures.

## Checklist

- [ ] Tray icon artwork + tooltip with live summary
- [ ] Full context menu (pause, placement, modules, settings, dashboard, about, exit)
- [ ] Single instance: named mutex + `RegisterWindowMessageW` hand-off
- [ ] Autostart: `HKCU\...\Run` (portable) / `StartupTask` (packaged)
- [ ] Settings load/save wired, debounced, live-applied
- [ ] `--tray-only`, `--safe-mode`, `--settings`, `--version`, `--help`
- [ ] Log rotation (1 MB × 3), error-only by default
- [ ] Panic hook + `SetUnhandledExceptionFilter` → log + tray notification
- [ ] Power throttling (EcoQoS) when hidden/fullscreen

## Exit criteria

Second launch focuses the first and exits; autostart works in both packaging modes;
killing the process mid-write never corrupts settings; log never grows unbounded.
