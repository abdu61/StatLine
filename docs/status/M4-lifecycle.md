# M4 — tray, lifecycle, settings I/O

**Status:** in progress (single instance + autostart + logging done)

## Done

- [x] Single instance via a named mutex (`StatLine.SingleInstance`); the second
      launch logs and exits. Note: a plain name is per-session, which is what we want —
      `Local\` prefixes need careful escaping and add nothing here.
- [x] `GetLastError` is cleared immediately before `CreateMutexW`; otherwise a stale
      error from any earlier startup call reads as "already exists".
- [x] Autostart through `HKCU\...\Run` with `--install-startup`, `--remove-startup`,
      `--startup-status`. Verified round-trip: disabled → enabled → disabled.
- [x] Log rotation: 1 MB cap, three rotated files, done inside the logger under the
      same lock as the write.
- [x] Panic hook records file:line before the abort.
- [x] `--tray-only` to run without the strip.

## Remaining

- [ ] Tray menu: Pause/Resume, Show/Hide strip, Settings, Dashboard, About, Exit
- [ ] Sampler pause (stop periodic sampling, keep the last snapshot)
- [ ] Second launch hands its arguments to the first instance (open Settings)
- [ ] Startup preference persisted in settings; first-run prompt
- [ ] `--safe-mode` (force topmost-only behaviour, no taskbar anchoring attempts)

## Commands

```powershell
statline --startup-status
statline --install-startup
statline --remove-startup
statline --tray-only
```
