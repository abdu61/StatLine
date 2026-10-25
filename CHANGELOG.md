# Changelog

All notable changes to Statline are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The companion remembers its position and size in `window.json` and only restores a
  rectangle that still intersects a monitor. Closing it with X now lets the Dashboard
  menu open it again.
- System page shows the Windows edition/build and computer name (registry, read once),
  alongside process count, thermal state and battery summary.
- Memory page gains an Available row; the network card shows its link speed; the bench
  gate now uses the wall-clock sample time instead of the scheduler-quantized CPU time.
- Per-process rows are grouped by executable, so Chrome, svchost and friends appear once
  with summed CPU and memory.
- Memory page shows commit, **swap/page-file** usage (`SystemPageFileInformation`) and
  cache; network rows show link speed (e.g. "Wi-Fi · 866 Mbps"); the battery page shows
  time left, rate and design/full capacity.
- Processor name and base clock from the registry, shown on the CPU card.
- GPU vendor telemetry loaded lazily from the installed driver: NVML for NVIDIA and ADL
  PMLog/Overdrive5 for AMD. Temperature, fan, power and clocks appear on the GPU page
  and in `--probe`. Verified on this AMD APU (Radeon graphics, 56 °C, 4 W, 400 MHz /
  1.2 GHz).
- CPU core grid with per-core percentage and clock from `CallNtPowerInformation`, in the
  flyout (two columns) and companion (four columns).
- Per-module strip visibility in Settings (CPU, memory, GPU, disk, network); the flyout
  and companion stay complete.
- About page in the companion: version, privacy posture, settings/log folders, licence
  and the sensor scope.
- CPU 1/5/15-minute smoothed averages (real elapsed time, not fixed ticks), shown on the
  CPU page and in `--probe`.
- Battery cycle count on the battery card when the firmware reports it.
- Accent presets (System, Blue, Green, Purple, Orange, Teal, Pink) applied to the strip,
  flyout, companion and settings; System reads the Windows DWM accent.
- Charts and meters use the accent normally and switch to warning yellow or critical
  red as a value approaches its limit (CPU/MEM 85/95, GPU/DISK 90/98).
- Mica warning in Settings: DWM blur can cost GPU power and battery. The flyout panel
  becomes lightly translucent when Mica is on.
- Binary/decimal units setting, applied live to the strip, flyout and companion.
- Companion left navigation: Dashboard, CPU, Memory, GPU, Disk, Network, Battery,
  System and Settings, with a dedicated detail page per module.
- Optional Mica window effect on the framed companion and settings windows
  (`appearance.mica`).
- Module-focused flyout: clicking a segment shows only that module's detail (per-core
  CPU bars, memory commit/cache, per-volume and per-adapter rows) plus System and
  Battery. CPU and Memory include their matching process tables.
- Network segment moved to the front of the strip so its variable rate text no longer
  shifts the other segments.

### Fixed

- The flyout now opens at its final anchored position and grows upward when the process
  table arrives, instead of appearing under the strip or being clipped.

## [0.1.0] - 2026-10-24

### Added

- Taskbar strip, parented into `Shell_TrayWnd`: CPU, memory, GPU, disk and two-line
  network readouts with per-segment hover and a right-click menu. Falls back to a
  topmost popup if the shell refuses the parent link.
- Flyout on strip click with 60-second area charts, per-module cards, the top five
  processes, Esc/click-away dismissal, and segment focus (MEM sorts by working set).
- Companion window: framed, resizable, with a card grid and an eight-row process table.
- Per-app breakdown from `NtQuerySystemInformation`, sampled at 2 s only while a
  detail surface is visible.
- Driver-free sensors: ACPI system temperature (PDH), drive temperature
  (`IOCTL_STORAGE_QUERY_PROPERTY`), battery health and temperature
  (`IOCTL_BATTERY_QUERY_INFORMATION`). Every source hides when unavailable.
- Settings window: theme, sampler interval, strip placement and start with Windows;
  applied live and persisted atomically to `%APPDATA%\Statline\settings.json`.
- Tray menu with pause/resume, show/hide strip, dashboard, settings and exit.
- Single instance, autostart commands, rotating log, panic hook, `--probe`, `--bench`,
  `--profile`, `--render-snapshots`, `--tray-only`.
- Packaging: portable zip script, MSIX manifest with `runFullTrust` and a startup task,
  CI for fmt/clippy/test/deny/build, README with screenshots, third-party notices.

### Performance

- 0.46 ms CPU per sample with detail providers off (tight loop); about 2.3 ms/sample
  while a detail surface is open, dominated by the process table.
- 30 MB working set / 14.5 MB private with the strip live; flat handles over a soak.
