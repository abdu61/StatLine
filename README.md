<div align="center">

<img src="docs/images/icon.png" alt="StatLine" width="88">

# StatLine

**A system monitor that lives in your Windows 11 taskbar.**

CPU, memory, GPU, disk and network in a 240-pixel strip. Native Rust, no Electron.

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange?logo=rust)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Windows%2011%20x64-0078D4)](https://www.microsoft.com/windows/windows-11)
[![Licence](https://img.shields.io/badge/licence-MIT-blue)](LICENSE)
[![Telemetry](https://img.shields.io/badge/telemetry-none-brightgreen)](PRIVACY.md)
[![Install](https://img.shields.io/badge/install-download%20%26%20setup-2ea043?logo=windows11&logoColor=white)](#install)

</div>

<p align="center">
  <img src="docs/images/strip.jpg" alt="StatLine in the taskbar" width="520">
</p>

## Why

Most taskbar monitors are either an Electron app idling at 150 MB, or a widget that
needs a kernel driver to read a temperature. StatLine is neither: one 680 KB executable
using the Windows performance counters and nothing else.

## Features

- **Taskbar strip** — CPU, memory, GPU, disk and a two-line network readout, anchored
  beside the tray. It is a real child of `Shell_TrayWnd`, so the Start menu cannot cover
  it and clicking it never steals focus.
- **Flyout** — click a segment for that module alone: per-core clocks, commit and cache,
  VRAM, per-volume capacity, per-adapter rates.
- **Companion window** — a normal resizable window with a page per module and a
  remembered position.
- **Sensors without a driver** — ACPI thermal zone, drive temperature, battery health and
  cycle count, and GPU telemetry via NVML or ADL when the driver exposes it. Wi-Fi name
  and signal are **off by default**: reading the SSID makes Windows ask for location
  permission, so it is opt-in in Settings and explained there first. Anything unavailable
  is hidden, never faked.
- **Per-app detail** — CPU and GPU share per process, grouped by executable. Sampling runs
  only while a surface that shows it is open.
- **Yours to configure** — theme, seven accent colours, interval, binary or decimal units,
  °C or °F, strip density, module order, Mica backdrop, start with Windows.

<table>
<tr>
<td><img src="docs/images/flyout.jpg" alt="Flyout" width="300"></td>
<td><img src="docs/images/dashboard-cpu.jpg" alt="CPU page" width="420"></td>
</tr>
<tr>
<td><img src="docs/images/dashboard.jpg" alt="Dashboard" width="420"></td>
<td><img src="docs/images/settings.jpg" alt="Settings" width="300"></td>
</tr>
</table>

## Install

<p>
  <a href="https://github.com/abdu61/StatLine/releases/latest/download/StatLine-0.1.0-x64.zip">
    <img src="https://img.shields.io/badge/Download-StatLine%200.1.0%20for%20Windows%2011-2ea043?style=for-the-badge&logo=windows11&logoColor=white" alt="Download StatLine for Windows 11">
  </a>
</p>

Unpack the zip anywhere and run `statline.exe`. No installer, no administrator rights, no
runtime to install.

Each release publishes a `.sha256` next to the zip, so you can verify the download:

```powershell
Get-FileHash .\StatLine-0.1.0-x64.zip -Algorithm SHA256
```

> **Two things to expect on first run.** The binaries are not code-signed yet, so Windows
> SmartScreen shows "Windows protected your PC" — choose **More info → Run anyway**, or
> build it yourself below. And the app asks for nothing: it runs as a standard user and
> never requests elevation.

To start with Windows, enable **Start with Windows** in Settings — it writes a per-user
`Run` value. Verify or remove it from a console:

```powershell
statline.exe --startup-status
statline.exe --remove-startup
```

## Build

Requires Windows 11, Rust MSVC 1.85+ and the Visual Studio C++ build tools (for
`link.exe`). Details in [`docs/ops/build.md`](docs/ops/build.md).

```powershell
cargo build --release
.\target\release\statline.exe
```

Before sending a patch:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo deny check
```

## Privacy

No network access. No telemetry, no analytics, no update check. No kernel driver and no
elevated helper.

| | |
|---|---|
| **Reads** | Performance counters, PDH, DXGI, storage and battery IOCTLs, SMBIOS, and read-only registry (theme, CPU name, Windows build) |
| **Writes** | `%APPDATA%\StatLine` (settings), `%LOCALAPPDATA%\StatLine\logs`, and an optional per-user `Run` value |
| **Never** | Admin/UAC, network, telemetry, drivers or services, clipboard, input, screen or document access |
| **Location** | Only if you turn on Wi-Fi details. Windows gates the network name behind a location permission prompt; with it off — the default — the app never asks and never reads the SSID |

The full read/write table is in [`docs/ACCESS.md`](docs/ACCESS.md); the policy is in
[`PRIVACY.md`](PRIVACY.md).

## What it deliberately does not do

- **No CPU package temperature or fan RPM.** Windows has no user-mode API for these;
  reading them needs a kernel driver, which is out of scope for a taskbar utility.
- **No network of any kind.** Not even an update check.
- **No code copied from other monitor projects.** Win32 usage is written from the public
  API documentation.

## Contributing

Issues and pull requests are welcome. Read [`AGENTS.md`](AGENTS.md) first — it describes
the resource budget, the `unsafe` rules and the verification gate. The short version:
things must stay fast, and a claim about Windows behaviour needs the API named.

## Licence

MIT — see [`LICENSE`](LICENSE). Third-party notices are in
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
