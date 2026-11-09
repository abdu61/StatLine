# Ops — release checklist

Run top to bottom. Do not skip the parts that need eyes.

## Code

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test --all`
- [ ] `cargo deny check`
- [ ] `cargo build --release`
- [ ] No `TODO`/`FIXME` left in shipping paths (grep and justify each)
- [ ] Version bumped in `Cargo.toml` + `CHANGELOG.md`
- [ ] `THIRD_PARTY_NOTICES.md` regenerated

## Visual

- [ ] Snapshots regenerated and reviewed: light/dark × 100/150/200 %
- [ ] Live capture of the strip on this machine (`scripts/capture-window.ps1`)
- [ ] Dashboard and flyout screenshots reviewed
- [ ] Tooltip, menu, empty state, unavailable-module state all look correct

## Behaviour

- [ ] Compatibility matrix in `testing.md` completed
- [ ] Explorer restart while running
- [ ] Clean profile: no settings file → defaults → first run works, access notification shown
- [ ] Uninstall path (portable: delete folder; MSIX: uninstall) leaves no residue
- [ ] Second instance focuses the first

## Startup and access

- [ ] Release `statline.exe --install-startup` writes `...\statline.exe` (never `target\debug`)
- [ ] `--startup-status` says `(matches this build)`
- [ ] Sign out/in: app appears, **no console window flashes**
- [ ] `cargo test` leaves `HKCU\...\Run\StatLine` untouched
- [ ] `docs/ACCESS.md` / `PRIVACY.md` match the build (no network, no admin, write paths)
- [ ] MSIX capabilities: `runFullTrust` + disabled startup task only

## Performance

- [ ] `--bench 300` recorded in `perf-log.md`, all budgets met
- [ ] 1-hour soak: handles, threads, working set stable
- [ ] Idle CPU checked in Task Manager with the strip visible

## Packaging

- [ ] `StatLine-<version>-x64.zip` built and unzipped on a clean path
- [ ] MSIX builds; installs via `Add-AppxPackage` with the dev cert
- [ ] Startup task present and toggleable
- [ ] LICENSE + THIRD_PARTY_NOTICES present in both artifacts

## Docs

- [ ] `docs/STATUS.md` and the milestone status file updated
- [ ] README screenshots current
- [ ] `docs/ACCESS.md` + `PRIVACY.md` current; publishing checklist reviewed
- [ ] Privacy statement still true: no network, no telemetry, no admin, no driver

## Tag (local only)

```powershell
git tag -a v<version> -m "StatLine v<version>"
```

Never push. Publishing is a separate, explicitly-approved step.
