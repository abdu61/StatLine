# M0 — toolchain & scaffold

**Status:** complete · 2026-10-24

## Goal

A repo that builds, runs, and has an indexed documentation set, before any product code.

## Done

- Rust 1.99.1 `stable-x86_64-pc-windows-msvc` installed user-scope (no admin).
- Workspace with 7 crates, `resolver = 2`, edition 2024, release profile LTO + strip.
- `rust-toolchain.toml` pins stable + rustfmt + clippy.
- Tray app: hidden message window, version-4 tray icon (`NOTIFYICON_VERSION_4`),
  custom-drawn-free Win32 popup menu, file logger with UTC timestamps.
- `statline-config`: versioned schema, atomic save, corrupt-file quarantine, 4 tests.
- 15 unit tests green; `clippy --all-targets -D warnings` clean in debug + release.
- Tooling verified against the real machine:
  - `probe-taskbar.ps1` → taskbar rect, tray rect, DPI, auto-hide state, XAML markers.
  - `capture-window.ps1` → `PrintWindow(PW_RENDERFULLCONTENT)` captures the taskbar,
    which `CopyFromScreen` does not.
- Docs: index, status, architecture, 5 ADRs, 5 specs, 5 ops docs, milestone files.

## Decisions

- Rust over C#: 4–8× less memory, no runtime (ADR 0001).
- Own layered child window over deskband/AppBar/injection (ADR 0002).
- Software D2D over D3D swapchain: zero idle GPU (ADR 0003).
- `%APPDATA%` settings, `%LOCALAPPDATA%` logs (ADR 0004).
- `unsafe` confined to `statline-win` + D2D calls in `statline-render`.

## Evidence

```
cargo test --quiet      → 15 passed, 0 failed
cargo clippy -D warnings → clean
app launch              → log: "Statline 0.1.0 starting" / "tray icon ready"
taskbar probe           → 1536x48 logical, dpi 120, TrayNotifyWnd at x=1272,
                          MSTaskSwWClass = 0
```

## Follow-ups

- Verify "Statline" is available in Partner Center + a trademark knock-out search
  before packaging (M9), not before M1–M8.
- Artwork: real app icon replaces `IDI_APPLICATION` in M4.
