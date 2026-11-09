# Access and permissions

What StatLine reads, writes, and never touches. This is the short, complete answer;
the privacy policy is in [`PRIVACY.md`](../PRIVACY.md).

StatLine runs as an ordinary user process. It never requests elevation, installs a
driver or service, or opens a network connection.

## Reads

| Capability | Used for | API |
|---|---|---|
| CPU time, per-core clocks | CPU card, core grid | `GetSystemTimes`, `NtQuerySystemInformation`, `CallNtPowerInformation` |
| Physical memory, commit, cache | Memory card | `GlobalMemoryStatusEx`, `GetPerformanceInfo` |
| Per-process CPU time, working set, names | Top processes | `NtQuerySystemInformation(SystemProcessInformation)` |
| GPU engines, VRAM | GPU card | PDH `\GPU Engine(*)`, DXGI `QueryVideoMemoryInfo` |
| Per-process GPU share | GPU page process table | same `\GPU Engine(*)` counter, PID parsed from the instance name |
| Disk throughput, per-disk | Disk card | PDH `\PhysicalDisk(*)` |
| Thermal zone | System card | PDH `\Thermal Zone Information(*)` |
| Drive temperature | Disk volume rows | `IOCTL_STORAGE_QUERY_PROPERTY` on the volume/disk |
| Drive model and bus type | Disk page | `IOCTL_STORAGE_QUERY_PROPERTY` (`StorageDeviceProperty`) on `\\.\PhysicalDriveN`, zero-access handle |
| Battery charge, health, temperature, cycles | Battery card | `GetSystemPowerStatus`, `CallNtPowerInformation`, `IOCTL_BATTERY_QUERY_INFORMATION` |
| Wi-Fi SSID and signal | Network page, **off by default** | `wlanapi` — see the location note below |
| GPU temperature/fan/power/clocks | GPU page | `nvml.dll` (NVIDIA) or `atiadlxx.dll` (AMD), loaded at runtime if present |
| Theme, accent, CPU name, base clock, Windows build, computer name | Appearance and About | read-only registry (`HKCU`/`HKLM`) |
| Installed memory modules: slot, size, DDR generation, speed | System page | `GetSystemFirmwareTable('RSMB')` (SMBIOS type 17, read once at first sample) |

Registry keys read:

- `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize` — app theme
- `HKCU\Software\Microsoft\Windows\DWM` — accent colour
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced` — `TaskbarAl`
  (taskbar alignment) and related layout values
- `HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0` — processor name and base clock
- `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion` — Windows edition/build
- `HKLM\SYSTEM\CurrentControlSet\Control\ComputerName\ComputerName` — computer name

StatLine reads **metadata**, never document contents, browser history, clipboard,
keystrokes, or screen pixels.

## Location, and why Wi-Fi is opt-in

Windows exposes the Wi-Fi network name only through `WlanQueryInterface` with
`wlan_intf_opcode_current_connection`, and it treats that as a **location request**: the
first call raises the system prompt *"Let StatLine use your location?"*.

A new system monitor asking for location on first launch is how you get uninstalled, so:

- The write is **off by default** (`general.wifi_details` in `settings.json`).
- The Settings row says what will happen *before* you click it, and the warning sits under
  the row while the setting is on.
- Turning it off clears the cached SSID immediately.
- With it off, `WlanQueryInterface` is never called — the location prompt never appears.

StatLine does not use GPS, cell triangulation or the Windows location API. It reads the
name of the network you are already connected to, and nothing else.

## Writes

| Path | When |
|---|---|
| `%APPDATA%\StatLine\settings.json` | settings change (atomic write) |
| `%APPDATA%\StatLine\window.json` | companion moved/resized/closed |
| `%LOCALAPPDATA%\StatLine\logs\statline.log*` | normal logging, rotating at 1 MB × 3 |
| `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\StatLine` | only when "Start with Windows" is enabled |

Nothing else is written. Portable builds keep no files outside those directories.
Uninstalling is deleting the folder and, if enabled, the `Run` value (Settings →
Start with Windows, or `statline.exe --remove-startup`).

## Startup

"Start with Windows" writes a per-user `Run` value:

```
"HKCU\...\Run"  StatLine = "C:\path\to\statline.exe"
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
- No writing outside `%APPDATA%\StatLine`, `%LOCALAPPDATA%\StatLine`, and the optional
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

# remove everything this app stored (settings + window position)
.\statline.exe --reset-settings

# run with only documented CPU/memory/system counters
.\statline.exe --safe-mode

# who am I / integrity level: ordinary user, medium integrity
whoami /groups | findstr /i "Mandatory"
```

The Settings window and About page repeat the same summary; the first launch shows a
one-time tray notification pointing here. Focus Assist can suppress that notification.
