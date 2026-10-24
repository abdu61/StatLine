# M9 — packaging & release

**Status:** not started

## Goal

Something a stranger can install, trust, and uninstall cleanly.

## Checklist

- [ ] Store + trademark availability check for "Statline"; reserve the name
- [ ] Real app icon (svg → multi-size ico) and MSIX tile assets
- [ ] `packaging/msix/AppxManifest.xml` with `runFullTrust` + `windows.startupTask`
- [ ] `scripts/build-msix.ps1` (makeappx + optional self-signed cert for testing)
- [ ] Portable zip script with LICENSE + THIRD_PARTY_NOTICES + README
- [ ] CI: fmt, clippy, test, deny, release build, artifacts
- [ ] Release workflow: tag → zip + msix + SHA-256
- [ ] README with screenshots, install steps, privacy statement
- [ ] `CHANGELOG.md`, `LICENSE` (MIT), `THIRD_PARTY_NOTICES.md`
- [ ] Final `--bench 300` + 1-hour soak recorded
- [ ] Local annotated tag `v0.1.0` (never pushed)

## Exit criteria

Clean-profile install works in both channels; uninstall leaves no residue; every item
in `ops/release-checklist.md` is ticked with evidence.
