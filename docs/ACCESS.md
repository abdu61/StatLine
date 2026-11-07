# Access and permissions

What Statline reads, writes, and never touches. This is the short, complete answer;
the privacy policy is in [`PRIVACY.md`](../PRIVACY.md).

Statline runs as an ordinary user process. It never requests elevation, installs a
driver or service, or opens a network connection.

## Reads

| Capability | Used for | API |
|---|---|---|
| CPU time, per-core clocks | CPU card, core grid | `GetSystemTimes`, `NtQuerySystemInformation`, `CallNtPowerInformation` |
| Physical memory, commit, cache | Memory card | `GlobalMemoryStatusEx`, `GetPerformanceInfo` |
| Per-process CPU time, working set, names | Top processes | `NtQuerySystemInformation(SystemProcessInformation)` |
| GPU engines, VRAM | GPU card | PDH `\GPU Engine(*)`, DXGI `QueryVideoMemoryInfo` |
| Disk throughput, per-disk | Disk card | PDH `\PhysicalDisk(*)` |
| Thermal zone | System card | PDH `\Thermal Zone Information(*)` |
| Drive temperature | Disk volume rows | `IOCTL_STORAGE_QUERY_PROPERTY` on the volume/disk |
| Battery charge, health, temperature, cycles | Battery card | `GetSystemPowerStatus`, `CallNtPowerInformation`, `IOCTL_BATTERY_QUERY_INFORMATION` |
| Wi-Fi SSID and signal | Network page | `wlanapi` (only while a detail surface is open) |
| GPU temperature/fan/power/clocks | GPU page | `nvml.dll` (NVIDIA) or `atiadlxx.dll` (AMD), loaded at runtime if present |
| Theme, accent, CPU name, base clock, Windows build, computer name | Appearance and About | read-only registry (`HKCU`/`HKLM`) |

Registry keys read:

- `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize` — app theme
- `HKCU\Software\Microsoft\Windows\DWM` — accent colour
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced` — `TaskbarAl`
  (taskbar alignment) and related layout values
- `HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0` — processor name and base clock
- `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion` — Windows edition/build
- `HKLM\SYSTEM\CurrentControlSet\Control\ComputerName\ComputerName` — computer name

Statline reads **metadata**, never document contents, browser history, clipboard,
keystrokes, or screen pixels.

## Writes

| Path | When |
|---|---|
| `%APPDATA%\Statline\settings.json` | settings change (atomic write) |
| `%APPDATA%\Statline\window.json` | companion moved/resized/closed |
| `%LOCALAPPDATA%\Statline\logs\statline.log*` | normal logging, rotating at 1 MB × 3 |
| `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Statline` | only when "Start with Windows" is enabled |

Nothing else is written. Portable builds keep no files outside those directories.
Uninstalling is deleting the folder and, if enabled, the `Run` value (Settings →
Start with Windows, or `statline.exe --remove-startup`).

## Startup

"Start with Windows" writes a per-user `Run` value:

```
"HKCU\...\Run"  Statline = "C:\path\to\statline.exe"
```

No arguments, no elevation, no scheduled task, no service. It is optional and off by
default. Verify or remove it from a console:

```powershell
statline.exe --startup-status
statline.exe --remove-startup
```

The value is only ever written by a **release** executable named `statline.exe`.
Debug builds and test harnesses are refused, so `cargo test` cannot put a console test
binary into your login. If an older build left a stale path, the app repairs it on the
next launch when the setting is on.

## Never

- No administrator/UAC elevation.
- No network connections, update checks, telemetry, or analytics.
- No kernel driver, service, scheduled task, or shell extension.
- No keyboard, mouse, clipboard, screen, or document access.
- No writing outside `%APPDATA%\Statline`, `%LOCALAPPDATA%\Statline`, and the optional
  per-user `Run` value.
- No access to other users' profiles.

## Packaged build (MSIX)

The manifest declares only `runFullTrust` (required for a full-trust desktop app) and a
startup task that is **disabled by default**. It does not request `internetClient`,
`broadFileSystemAccess`, or any device capability.

## Verify it yourself

```powershell
# what is registered for login, and does it match this build?
.\statline.exe --startup-status

# live readings (CPU, memory, disk, GPU, network, sensors)
.\statline.exe --probe

# who am I / integrity level: ordinary user, medium integrity
whoami /groups | findstr /i "Mandatory"
```

The Settings window and About page repeat the same summary; the first launch shows a
one-time tray notification pointing here. Focus Assist can suppress that notification.
