# Status

**Milestone:** M0 — toolchain & scaffold · **Updated:** 2026-10-24

## Now

- [x] Rust 1.99.1 MSVC installed (winget), links with existing VS 2022 toolchain
- [x] Workspace scaffolded (7 crates), builds clean
- [x] Tray app: hidden message window, tray icon, context menu, file logger
- [x] 15 unit tests green; `clippy -D warnings` clean
- [x] Taskbar probe + `PrintWindow` capture tooling verified on this machine
- [ ] Product-name availability check (Statline)
- [ ] Docs tree, ADRs, CI stub, `deny.toml`
- [ ] First commit

## Next (in order)

1. Finish M0 docs + CI + `deny.toml`, then commit.
2. M1: CPU/memory providers → `--probe` → `--bench`.
3. M1: disk/network providers, then GPU (PDH) and battery.
4. M2: Direct2D software surface + headless snapshot harness.
5. M3: taskbar strip window lifecycle.

## Blockers / decisions needed

- None. "Vitals" was rejected (Store collision); **Statline** chosen — Store + trademark
  availability to be verified before packaging (does not block M1–M8).

## Measured facts (this machine)

Windows 11 25H2, build 26220.9568 · 1536×864 logical @ 125 % (DPI 120) · one monitor ·
taskbar bottom, **left-aligned**, 48 logical px tall · tray at right edge ·
`MSTaskSwWClass` absent (XAML taskbar) · `CopyFromScreen` misses the taskbar,
`PrintWindow(PW_RENDERFULLCONTENT)` captures it.

## Log

- 2026-10-24 — M0: workspace builds; tray app runs and logs; capture/probe tooling verified.
