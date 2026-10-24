# Status

**Milestone:** M3 complete (tier 1) · M4 complete · M5/M6 in progress ·
**Updated:** 2026-10-24

## Where it stands

The strip is a parented child of `Shell_TrayWnd`, the flyout opens and closes cleanly,
and the per-app breakdown is in: clicking the strip shows the busiest processes by CPU
share and working set, and the companion window shows the same table. The process query
only runs while one of those surfaces is open.

```
strip hosted by Shell_TrayWnd (tier 1)
strip 237x42 at (1348,1028) presented=true tier=child
process table sampling on
```

## Working

- **Strip**: CPU / MEM / GPU / DISK / NET with meters, hover plate per segment,
  right-click menu, parented into the taskbar (`WS_CHILD`), `WM_MOUSEACTIVATE`
  suppressed, hides for fullscreen and a hidden taskbar, recreates after an Explorer
  restart via the app message window. Topmost popup remains as an automatic fallback.
- **Flyout**: opens on strip click, toggles closed on a second click, closes on Esc or
  focus loss, renders smoothed area charts, and lists the top five processes with CPU
  share and working set.
- **Companion**: framed resizable Windows 11 window with a two-column card grid and an
  eight-row, two-column process table.
- **Per-app breakdown**: `NtQuerySystemInformation(SystemProcessInformation)` with CPU-time
  deltas; Task Manager scale (share of all cores); `—` until a baseline exists; sampled
  at 2 s only while the flyout or companion is open (~4.75 ms per query).
- **Sampling**: 0.78 ms per sample at 1 Hz (0.078 % of one core) without the process
  table, WS ~30 MB, private ~14.5 MB, handles flat.
- **Lifecycle**: single instance, autostart commands, 1 MB × 3 log rotation, panic
  hook, `--tray-only`.

## Next, in order

1. **Settings UI** (`statline-config` is written but unwired): theme, interval,
   placement, units, modules, autostart, safe mode; live-apply.
2. **Segment focus**: clicking CPU/MEM/GPU opens the flyout focused on that module
   (highlight the card, sort the process table by CPU or memory accordingly).
3. **Driver-free sensors (M7)**: GPU temp/fan/power, drive temps, battery health,
   ACPI thermal zone. Degrade to hidden; never fake a value.
4. **Packaging (M9)**: MSIX, portable zip, CI, README with snapshots, notices,
   local `v0.1.0` tag.
5. **Polish**: tooltips, keyboard path, empty states, `--safe-mode`, multi-monitor,
   `WM_DPICHANGED`, Mica on the companion.

## Measured

| Mode | WS | Private | Handles |
|---|---|---|---|
| `--tray-only` | 10.6 MB | 1.8 MB | 145 |
| strip live, parented (tier 1) | 30.0 MB | 14.5 MB | 314 |
| companion + process table | 38.9 MB | 17.9 MB | 374 |

The process table query costs 4.75 ms for ~250 processes; at the 2 s cadence and only
while a process surface is open that is ~0.24 % of one core, and zero when closed.
Handles and private bytes are flat across companion open/close cycles.

## Log

- M0 scaffold · M1 metrics · M2 renderer · M3 strip live, parented into the taskbar ·
  M4 lifecycle · M5 flyout verified, top processes · M6 companion window + process table.
