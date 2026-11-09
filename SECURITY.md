# Security policy

StatLine runs as an ordinary user process. It opens no sockets, installs no driver or
service and requests no elevation. The realistic attack surface is small, but it is not
zero: it parses a settings file, loads vendor GPU libraries at runtime, and issues device
IOCTLs.

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting on this repository
(**Security** → **Report a vulnerability**) rather than a public issue.

Include the version (`statline.exe --version`), your Windows build, and a description of
the impact. A proof of concept helps but is not required.

I aim to acknowledge within a week. This is a spare-time project, so please do not expect
a same-day fix.

## Scope

In scope:

- Memory-safety errors in the `unsafe` Win32 / Direct2D code
- A crafted `settings.json` or `window.json` causing memory corruption or arbitrary code
  execution
- Path traversal or unintended writes outside `%APPDATA%\StatLine` and
  `%LOCALAPPDATA%\StatLine`
- DLL planting via the runtime-loaded `nvml.dll` / `atiadlxx.dll`

Out of scope:

- Anything requiring administrator rights
- Denial of service through extreme system configuration
- Reports that a monitor app can read system performance counters — that is the feature

## Supported versions

Only the latest release receives fixes during the 0.x preview.
