# M9 — packaging & release

**Status:** in progress · portable zip, MSIX, access/privacy docs and icon; release soak and tag pending

## Goal

Something a stranger can install, trust, and uninstall cleanly.

## Checklist

- [ ] Store + trademark availability check for "Statline"; reserve the name (human)
- [x] MSIX tile assets (44/50/150/310x150/620x300 PNGs) regenerated from the icon mark
- [x] Multi-size `.ico` embedded in the executable by `build.rs` (window/tray icon)
- [x] First-run access notification; `docs/ACCESS.md` + `PRIVACY.md`; About Access block
- [x] Startup entry guarded to release `statline.exe`, stale values repaired, tests
      cannot write the `Run` key; `--startup-status` reports the exact command
- [x] `packaging/msix/AppxManifest.xml` with `runFullTrust` + `windows.startupTask`
- [x] `scripts/build-msix.ps1` (makeappx + optional self-signed cert); verified locally,
      unsigned package produced
- [x] Portable zip script with LICENSE + PRIVACY + ACCESS + THIRD_PARTY_NOTICES +
      README + CHANGELOG + licence texts; verified
      (`dist/Statline-0.1.0-x64.zip`, SHA-256 recorded)
- [x] CI: fmt, clippy, test, deny, release build, snapshot artifact; tag job produces
      zip + MSIX + SHA-256 and uploads them
- [x] README with screenshots, install steps, privacy statement
- [x] `CHANGELOG.md`, `LICENSE` (MIT), `THIRD_PARTY_NOTICES.md`, `licenses/`
- [x] Final `--bench 300` recorded (median 1.27 ms, WS 13.3 MB, within budget)
- [ ] 1-hour soak recorded
- [ ] Local annotated tag `v0.1.0` (never pushed)

## Evidence

```
dist/Statline-0.1.0-x64.zip        255 685 bytes
  statline.exe                     481 792 bytes
  LICENSE, README.md, CHANGELOG.md, THIRD_PARTY_NOTICES.md, licenses/{MIT,Apache-2.0}.txt
dist/Statline-0.1.0-x64.msix       270 321 bytes   (unsigned, makeappx succeeded)
SHA-256 2e22f3fbc9b99f4ad5ea9b9c571f66ef3700e5c1393fd06805fc84c91a95417a
```

## Exit criteria

Clean-profile install works in both channels; uninstall leaves no residue; every item
in `ops/release-checklist.md` is ticked with evidence.
