# Ops — packaging

Two channels, one binary.

## Portable (GitHub releases)

```
dist/Statline-<version>-x64/
  statline.exe
  LICENSE
  THIRD_PARTY_NOTICES.md
  README.md
```

Zip as `Statline-<version>-x64.zip`. No installer, no registry writes except the
optional `HKCU\...\Run` autostart value toggled by the user.

## MSIX (Microsoft Store and sideload)

`packaging/msix/AppxManifest.xml`:

- `runFullTrust` desktop capability
- `windows.startupTask` extension (user-toggleable in Task Manager → Startup)
- tile + logo assets at every required scale
- identity: `Publisher="CN=Statline"`, `Name="Statline"`, `Version` from `Cargo.toml`

Build with the SDK's `makeappx.exe`:

```powershell
.\scripts\build-msix.ps1 -Version 0.1.0
```

Signing:

- **Store:** Microsoft re-signs after certification; no local certificate needed.
- **Sideload/testing:** self-signed dev certificate, documented in the script, never
  committed (`.pfx`/`.cer` are gitignored).

## Rules

- The binary must behave identically in both channels; only autostart mechanics differ
  (`GetCurrentPackageFullName` decides).
- No writes outside `%APPDATA%`, `%LOCALAPPDATA%` and `HKCU\...\Run`.
- No network calls, so packaging needs no firewall or capability declarations.
- `THIRD_PARTY_NOTICES.md` is generated from the dependency allowlist and shipped in
  both channels.

## winget

`packaging/winget/` holds a draft manifest pointing at the GitHub release asset.
Submission is a separate, later step and requires a stable release URL.
