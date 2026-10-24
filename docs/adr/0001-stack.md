# ADR 0001 — Rust + windows-rs for a native taskbar monitor

**Status:** accepted · 2026-10-24

## Context

An always-on monitor must cost almost nothing: low memory, near-zero CPU, no GPU
device held open, no runtime to install. The taskbar integration needs raw Win32
(window parenting, layered windows, DPI, shell messages), so whichever language wins
must make Win32 interop pleasant and safe.

## Options

| Option | Working set | Notes |
|---|---|---|
| C# .NET 9 + WinUI 3 | 120–200 MB | Fast to build UI, but the runtime and XAML stack dominate cost. |
| C# .NET 9 + WinForms/WPF | 60–120 MB | Better, still heavy; NativeAOT does not support either UI stack. |
| Electron/Tauri | 80–150 MB | WebView2 for a 60-pixel strip: wrong tool. |
| C++ + Win32 | 15–25 MB | Smallest, but manual lifetime management across a large COM/Win32 surface. |
| **Rust + windows-rs** | **~15–30 MB** | First-party bindings, safe wrappers, no runtime, single exe. |

Toolchain check on the target machine: MSVC 14.42 + Windows SDK 10.0.19041 + VS 2022
Community already installed; Rust installs user-scope with no admin.

## Decision

Rust (edition 2024, stable) with the `windows` crate. `unsafe` is confined to
`statline-win` and the Direct2D calls in `statline-render`; every block carries a
`// SAFETY:` justification.

## Consequences

- ~4–8× lower memory than the C# options, no runtime dependency, one signed exe.
- We write our own widget kit instead of getting one from XAML — more code, total
  control over cost and appearance.
- Rust's compile times are slower than C#'s incremental builds; the workspace is split
  into small crates so day-to-day rebuilds stay fast.
- Windows-only build target: fine, this is a Windows-only product.
