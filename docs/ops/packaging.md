# Ops — packaging

Two channels, one binary.

## Portable (GitHub releases)

```
dist/StatLine-<version>-x64/
  statline.exe
  LICENSE
  PRIVACY.md
  THIRD_PARTY_NOTICES.md
  README.md
  docs/ACCESS.md
```

Zip as `StatLine-<version>-x64.zip`. No installer, no registry writes except the
optional `HKCU\...\Run` autostart value toggled by the user.

## MSIX (Microsoft Store and sideload)

`packaging/msix/AppxManifest.xml`:

- `runFullTrust` desktop capability; **no** `internetClient`, `broadFileSystemAccess`,
  or device capabilities
- `windows.startupTask` extension (user-toggleable in Task Manager → Startup), disabled
  by default
- tile + logo assets at every required scale
- identity: `Publisher="CN=StatLine"`, `Name="StatLine"`, `Version` from `Cargo.toml`
- `PRIVACY.md` and `docs/ACCESS.md` staged next to the binary

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

Not started. A winget manifest needs a stable, public release URL first, so it is a
post-1.0 step: publish a GitHub release, then add `packaging/winget/` pointing at that
asset. Nothing in the repository references a manifest yet.
