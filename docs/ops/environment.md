# Ops — reference environment

The machine the project is being developed and visually verified on. Record facts here
so future sessions do not re-derive them.

| Fact | Value |
|---|---|
| OS | Windows 11 25H2, build **26220.9568** (registry still says "Windows 10 Home" — ignore) |
| Session | interactive desktop, single monitor |
| Logical resolution | 1536 × 864 |
| Physical resolution | 1920 × 1080 |
| Scaling | **125 %** (DPI 120) |
| Taskbar | bottom edge, **left-aligned** (not centred), 48 logical px / 60 physical px tall |
| Taskbar classes | `Shell_TrayWnd` present, `TrayNotifyWnd` present, `Start` present, **`MSTaskSwWClass` absent** |
| Tray rect (logical) | `(1272, 816) – (1536, 864)` |
| Auto-hide | off |
| Capture | `CopyFromScreen` **misses** the taskbar; `PrintWindow(PW_RENDERFULLCONTENT)` captures it |
| Toolchain | Rust 1.99.1 MSVC, MSVC 14.42, SDK 10.0.19041, VS 2022 Community |

## Consequences for development

- 125 % scaling is the default test case, not 100 %. Check 100 % and 200 % separately.
- Left-aligned taskbar means the empty region is between the app icons and the tray —
  the strip's right-anchored placement is exercised by default here.
- A second monitor is not available locally; multi-monitor code paths must be written
  defensively and reviewed, not assumed tested.
- Screenshot-based QA must use `scripts/capture-window.ps1`.
