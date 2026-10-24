# Statline

A live system monitor that lives in the Windows 11 taskbar. Native Rust + Win32 +
Direct2D — no Electron, no WebView, no driver, no telemetry, no network access.

> **Status: early development.** The tray shell and the documentation set exist; the
> taskbar strip, flyout and dashboard are being built milestone by milestone. See
> [`docs/STATUS.md`](docs/STATUS.md) for exactly where things stand.

## What it will do

- Compact strip in the taskbar: CPU, memory, GPU, disk and network at a glance
- Click for a flyout with 60-second charts, per-core CPU, VRAM, per-volume disk,
  per-adapter network, battery and top processes
- A dashboard for deeper charts, reordering modules and configuring everything
- GPU temperature/fan/power and drive temperatures where Windows exposes them
  without a kernel driver

## What it deliberately does not do

- No kernel driver, no elevated helper, no third-party app dependency
- No network requests, no telemetry, no analytics
- No CPU package temperature or fan RPM — Windows has no user-mode API for those.
  The UI says so instead of showing a made-up number.

## Requirements

Windows 11 (build 22000 or later), x64.

## Build

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
cargo build --release
.\target\release\statline.exe
```

Requires the Rust MSVC toolchain and the Visual Studio C++ build tools (for `link.exe`).
See [`docs/ops/build.md`](docs/ops/build.md).

## Documentation

Everything lives in [`docs/`](docs/INDEX.md), indexed by task. Start with
[`docs/STATUS.md`](docs/STATUS.md).

## Licence

MIT. Third-party notices are generated into `THIRD_PARTY_NOTICES.md`.
