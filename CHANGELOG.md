# Changelog

All notable changes to Statline are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Binary/decimal units setting, applied live to the strip, flyout and companion.

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
