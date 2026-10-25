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

- **Strip**: network first, then CPU / MEM / GPU / DISK with meters, hover plate per
  segment, right-click menu, parented into the taskbar (`WS_CHILD`), `WM_MOUSEACTIVATE`
  suppressed, hides for fullscreen and a hidden taskbar, recreates after an Explorer
  restart via the app message window. Topmost popup remains as an automatic fallback.
  Network leads so its varying rate text changes the strip's left edge instead of
  shifting the other segments.
- **Flyout**: opens on strip click, toggles closed on a second click, closes on Esc or
  focus loss. A segment click shows only that module — CPU with per-core bars and the
  CPU process table, Memory with commit/cache and top memory apps, Disk with volumes,
  Network with adapters — plus System and Battery. Charts use the accent and turn
  warning yellow / critical red as a value approaches its limit. It is created at its
  final anchored rect and re-anchors upward when the process table arrives. With Mica
  on, the panel is lightly translucent.
- **Companion**: framed resizable window with a left nav (Dashboard, CPU, Memory, GPU,
  Disk, Network, Battery, System, Settings, About). Module pages show a labelled
  per-core grid (percentage and clock) plus 1/5/15-minute averages and processes;
  memory cache and top memory apps; per-volume and per-adapter rows. Optional Mica DWM
  backdrop on the framed windows.
- **Settings**: a framed window from the tray menu, the strip menu, or the companion nav,
  with theme, accent presets, sampler interval, byte units, Mica, strip placement,
  start-with-Windows and per-module strip visibility. Mica shows a GPU/battery caution.
  Changes apply live and are saved atomically to `%APPDATA%\Statline\settings.json`;
  unknown keys are ignored.
- **Packaging**: portable zip script (exe, README, changelog, licence texts) and an
  MSIX manifest with `runFullTrust` + a user-toggleable startup task. Both build locally;
  CI verifies fmt/clippy/test/deny/build and produces tag artifacts.
- **Per-app breakdown**: `NtQuerySystemInformation(SystemProcessInformation)` with CPU-time
  deltas; Task Manager scale (share of all cores); instances of the same executable are
  grouped into one row (Chrome, svchost); `—` until a baseline exists; sampled at 2 s
  only while the flyout or companion is open (~4.75 ms per query).
- **Sensors**: ACPI "System" temperature via PDH (5 s), drive temperatures via
  `IOCTL_STORAGE_QUERY_PROPERTY` (5 min cache, volume → disk mapping), battery health,
  temperature and cycle count via `IOCTL_BATTERY_QUERY_INFORMATION` (30 s). GPU vendor
  telemetry rides the 3 s GPU cadence: NVML for NVIDIA, ADL PMLog/Overdrive5 for AMD
  (temperature, fan, power, clocks). Every source degrades to hidden; they share the
  process table's surface-visibility gate.
- **CPU detail**: current load plus 1/5/15-minute exponentially smoothed averages
  computed from real elapsed time, a labelled core grid with each core's percentage and
  clock from `CallNtPowerInformation`, the registry processor name and nominal base clock.
- **Memory detail**: commit/cached plus aggregate page-file (swap) usage from
  `SystemPageFileInformation`, all converted from pages to bytes.
- **System detail**: Windows edition/build (corrected to "Windows 11" from the build
  number, since ProductName still says Windows 10 on many builds) and computer name,
  read once from the registry, plus process count.
- **Sampling**: 0.415 ms per sample in a tight loop with detail providers off; WS ~30 MB,
  private ~14.5 MB, handles flat. `--bench` uses wall-clock sample time (the per-sample
  CPU figure is scheduler-quantized); idle wall mean is ~1.4–2.0 ms at a 1 s cadence.
- **Lifecycle**: single instance, autostart commands, 1 MB × 3 log rotation, panic
  hook, `--tray-only`.

## Next, in order

1. **Settings, remaining**: module reorder, density, safe mode, remembered window
   bounds.
2. **Sensors out of reach**: CPU package/per-core temperature and fan RPM need a kernel
   driver; GPU vendor telemetry is implemented (NVML/ADL), so an NVIDIA machine is only
   needed to exercise that path.
3. **Packaging, remaining**: app `.ico`, icon in the executable/tray, a 1-hour soak and
   `--bench 300`, then the local `v0.1.0` tag.
4. **Polish**: tooltips, keyboard path, empty states, `--safe-mode`, multi-monitor,
   `WM_DPICHANGED`, per-pixel blur behind the flyout (needs a composition swapchain).

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
