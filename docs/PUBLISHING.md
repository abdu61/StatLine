# Publishing StatLine

Everything that must be true before a public release. Items marked **human** need an
account, certificate, or decision that cannot be automated locally.

## What ships

- `statline.exe` — native x64 Windows GUI binary, no runtime, no installer
- Portable zip: exe + README + CHANGELOG + LICENSE + THIRD_PARTY_NOTICES
- Optional MSIX: `packaging/msix/AppxManifest.xml` + generated tile assets + the same exe

Licence: MIT. No GPL/AGPL/LGPL/SSPL code; `deny.toml` enforces the allow-list.

## Pre-flight

- [ ] `docs/ACCESS.md` and `PRIVACY.md` match the build (read/write table, no network)
- [ ] README access section current; first-run notification reviewed
- [ ] Version in `Cargo.toml`, `CHANGELOG.md`, and MSIX `Identity Version` agree
- [ ] `cargo fmt --all -- --check`, `clippy -D warnings`, `cargo test --all`, `cargo deny check`
      (`cargo-deny` 0.20.x cannot resolve a workspace-inherited *path-only* dependency and
      reports `bug[unresolved-workspace-dependency]`; the workspace pins a `version` on each
      `statline-*` path dependency to keep the check green, which publishing would need
      anyway)
- [ ] `cargo build --release`
- [ ] Release PE subsystem is **Windows GUI** (no console flash):
      `$b=[IO.File]::ReadAllBytes('target\release\statline.exe'); $pe=[BitConverter]::ToInt32($b,0x3c); [BitConverter]::ToUInt16($b,$pe+24+68)` → `2`
- [ ] `--render-snapshots` reviewed; README screenshots refreshed
- [ ] `--bench 300` plus the 1-hour soak recorded (handles/threads/WS stable)

## Startup

- [ ] From the release exe: `statline.exe --install-startup`
- [ ] `statline.exe --startup-status` says `(matches this build)` and the path is
      `...\statline.exe` — never `target\debug\deps\...`
- [ ] `cargo test` leaves `HKCU\...\Run\StatLine` untouched (tests only read/refuse)
- [ ] Sign out/in (or reboot) once: app appears, **no console window flashes**
- [ ] `statline.exe --remove-startup` removes the value
- [ ] MSIX: the `StartupTask` is disabled by default and toggleable in Windows Settings

## Packaging

- [ ] `scripts/package.ps1` produces the zip and SHA-256; unzip on a clean path
- [ ] `makeappx` builds the MSIX; `Add-AppxPackage` works with a dev cert
- [ ] MSIX identity publisher matches the signing certificate (**human**)
- [ ] Capabilities are exactly `runFullTrust` + the startup task; no network/device caps
- [ ] LICENSE, THIRD_PARTY_NOTICES, PRIVACY and ACCESS are included in both artifacts
- [ ] App icon and tile assets are final (`scripts/make-icon.ps1`; the build embeds
      `packaging/statline.ico` when the Windows SDK `rc.exe` is available)
- [ ] Signature applied with `signtool` / Partner Center (**human**: certificate)
- [ ] Store listing: description, 5–8 screenshots, privacy policy URL, support URL,
      "no admin / no network / no telemetry" stated in the first paragraph

## Clean-machine test

- [ ] Windows 11 22H2+ ordinary user, no build tools, no GPU vendor SDK
- [ ] Portable zip runs; strip attaches; first-run access notification appears
- [ ] No settings file → defaults; settings save and survive restart
- [ ] Missing GPU/vendor data hides rows instead of showing zeros
- [ ] Uninstall (delete folder / MSIX) leaves no residue except optionally the Run value

## Tag

```powershell
git tag -a v0.1.0 -m "StatLine v0.1.0"
```

Local only. Publishing (push, Store submission, signing) requires explicit approval.

## Known gaps before 1.0

- Signing and the final Store identity require a certificate/Partner Center account.
- No NVIDIA hardware on the development machine; the NVML path is guarded but not
  exercised on real hardware.
- CPU package temperature/fan RPM need a kernel driver and are out of scope.
- The flyout cannot host a true DWM blur (layered window); it is translucent instead.
