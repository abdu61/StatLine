# Privacy policy

**StatLine 0.1.0 — effective 2026-11-07**

StatLine is a local system monitor. It does not collect, transmit, sell, or share any
data. There is no account, no telemetry, no analytics, and no network code in the
application. It works fully offline.

## What is processed

StatLine reads system performance and sensor data on your machine — CPU, memory, disk,
GPU, network, battery, thermal and vendor GPU readings — and displays it. Process
information is limited to executable names, CPU time, and working-set size. It never
reads documents, browsing history, clipboard contents, keystrokes, or screen pixels.

## What is stored

Only on your computer:

- `%APPDATA%\StatLine\settings.json` — your preferences
- `%APPDATA%\StatLine\window.json` — the companion window position and size
- `%LOCALAPPDATA%\StatLine\logs\statline.log*` — operational logs (rotating)
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\StatLine` — only if you enable
  "Start with Windows"

No data leaves the device. There is no crash reporting service; a panic is written to
the local log only.

## Network

The application contains no HTTP, socket, DNS, or update client. It never contacts
StatLine, GitHub, a vendor, or any third party. GPU vendor DLLs (`nvml.dll`,
`atiadlxx.dll`) are loaded from the installed display driver; they are not bundled and
are not contacted over the network by StatLine.

## Permissions

StatLine runs as a standard user with medium integrity. It never requests elevation and
installs no driver or service. The MSIX package declares only `runFullTrust` and a
disabled-by-default startup task. See [`ACCESS.md`](docs/ACCESS.md) for the full
read/write table.

## Children

StatLine is a utility with no user-generated content and no data collection. It is not
directed at children.

## Changes

Material changes to this policy will be noted in `CHANGELOG.md` and the release notes.
The policy always describes the exact build it ships with.

## Contact

Questions or reports: <https://commandcode.ai/discord> or the repository issue tracker.
