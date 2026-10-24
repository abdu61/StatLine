# Ops — testing

## Layers

| Layer | What it proves | Where |
|---|---|---|
| Unit | delta math, formatting, ring buffers, config migration, layout/hit-testing | `cargo test` |
| Provider smoke | each provider returns sane ranges on a real machine, never panics | `cargo test -- --ignored` (needs hardware) |
| Headless render | every surface renders; layout invariants hold at 100/150/200 % DPI | `--render-snapshots` |
| Live visual | the strip actually sits in the taskbar and survives real conditions | screenshot review |
| Perf | budget compliance | `--bench 300`, see `perf-budget.md` |
| Soak | no handle/thread/memory drift over an hour | `--bench 3600` + Task Manager |

## Visual QA loop

`CopyFromScreen` does **not** capture this machine's taskbar (composition limitation).
Use `PrintWindow` with `PW_RENDERFULLCONTENT`:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\capture-window.ps1
# writes %TEMP%\statline-taskbar.png and prints its path
```

Look at the PNG. Confirm: strip flush with the tray, correct 125 % scaling, text not
clipped, meters aligned, no stray border, both themes.

## Compatibility matrix (must all be exercised before a release)

| Axis | Values |
|---|---|
| DPI | 100 %, 125 % (this machine), 150 %, 200 % |
| Taskbar | bottom, top, auto-hidden, left-aligned, centred |
| Theme | light, dark |
| Monitors | 1, 2 (taskbar on both) |
| Shell | Explorer restart while running |
| Content | fullscreen video, presentation mode, screen locked/unlocked |
| Degradation | `--safe-mode`, `--tray-only`, module unavailable (e.g. no GPU counters) |

## Checklist — strip (M3)

- [ ] Appears within 250 ms of launch
- [ ] Sits between the app icons and the tray without overlapping either
- [ ] Follows taskbar auto-hide
- [ ] Hides for fullscreen video and returns afterwards
- [ ] Survives Explorer restart and DPI change
- [ ] Click does not deactivate the foreground window
- [ ] Values match Task Manager within ±2 % CPU, ±0.2 GB RAM
- [ ] No visible jitter between ticks (tabular figures)
- [ ] Idle CPU within budget while visible

## Checklist — lifecycle (M4)

- [ ] Second launch focuses the first instance; only one tray icon
- [ ] Autostart toggles the registry value in portable mode / StartupTask in MSIX
- [ ] Log rotates, never grows unbounded
- [ ] Force-kill during a write leaves settings intact (atomic write)
