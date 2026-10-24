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
detail sampling on
```

## Working

- **Strip**: CPU / MEM / GPU / DISK / NET with meters, hover plate per segment,
  right-click menu, parented into the taskbar (`WS_CHILD`), `WM_MOUSEACTIVATE`
  suppressed, hides for fullscreen and a hidden taskbar, recreates after an Explorer
  restart via the app message window. Topmost popup remains as an automatic fallback.
- **Flyout**: opens on strip click, toggles closed on a second click, closes on Esc or
  focus loss, renders smoothed area charts, and lists the top five processes with CPU
  share and working set. Clicking a segment focuses that card (accent bar); MEM sorts
  the process table by working set.
- **Companion**: framed resizable Windows 11 window with a two-column card grid and an
  eight-row, two-column process table.
- **Settings**: a framed window from the tray menu (or the strip menu) with theme,
  sampler interval, strip placement and start-with-Windows. Changes apply live and are
  saved atomically to `%APPDATA%\Statline\settings.json`; unknown keys are ignored.
- **Packaging**: portable zip script (exe, README, changelog, licence texts) and an
  MSIX manifest with `runFullTrust` + a user-toggleable startup task. Both build locally;
  CI verifies fmt/clippy/test/deny/build and produces tag artifacts.
- **Per-app breakdown**: `NtQuerySystemInformation(SystemProcessInformation)` with CPU-time
  deltas; Task Manager scale (share of all cores); `—` until a baseline exists; sampled
  at 2 s only while the flyout or companion is open (~4.75 ms per query).
- **Sensors**: ACPI "System" temperature via PDH (5 s), drive temperatures via
  `IOCTL_STORAGE_QUERY_PROPERTY` (5 min cache, volume → disk mapping), battery health and
  temperature via `IOCTL_BATTERY_QUERY_INFORMATION` (30 s). Every source degrades to
  hidden; they share the process table's surface-visibility gate.
- **Sampling**: 0.46 ms per sample in a tight loop with detail providers off; WS ~30 MB,
  private ~14.5 MB, handles flat.
- **Lifecycle**: single instance, autostart commands, 1 MB × 3 log rotation, panic
  hook, `--tray-only`.

## Next, in order

1. **Settings, remaining**: module enable/order, units, density/accent, safe mode,
   remembered window bounds, About pane.
2. **GPU vendor sensors (M7)**: NVML/ADL temperature, fan and power; current builds show
   utilization and VRAM only. This machine is AMD integrated, so an AMD discrete GPU is
   needed to verify the ADL path.
3. **Packaging, remaining**: app `.ico`, icon in the executable/tray, a 1-hour soak and
   `--bench 300`, then the local `v0.1.0` tag.
4. **Polish**: tooltips, keyboard path, empty states, `--safe-mode`, multi-monitor,
   `WM_DPICHANGED`, Mica on the companion.

## Measured

| Mode | WS | Private | Handles |
|---|---|---|---|
| `--tray-only` | 10.6 MB | 1.8 MB | 145 |
| strip live, parented (tier 1) | 30.0 MB | 14.5 MB | 314 |
| companion + process table | 38.9 MB | 17.9 MB | 374 |

The process table query costs 4.75 ms for ~250 processes; at the 2 s cadence and only
while a process surface is open that is ~0.24 % of one core, and zero when closed.
Handles and private bytes are flat across companion open/close cycles. Sensors are gated
the same way: the idle tight-loop sample is 0.46 ms, and opening a detail surface raises
the average to ~2.3 ms/sample (process table dominates).

## Log

- M0 scaffold · M1 metrics · M2 renderer · M3 strip live, parented into the taskbar ·
  M4 lifecycle · M5 flyout verified, top processes · M6 companion window + process table ·
  M7 drive/battery/ACPI sensors (GPU vendor sensors pending) · M9 portable zip + MSIX.
