# Status

**Milestone:** M1 complete · M2 renderer complete · M3 strip in progress · **Updated:** 2026-10-24

## Now

- [x] M0: workspace, tray shell, docs index, capture tooling
- [x] M1: CPU, memory, disk, network, GPU, battery, sampler thread
- [x] M2: Direct2D software surface, palette/type tokens, text engine,
      dependency-free PNG writer, `--render-snapshots` smoke test (verified visually)
- [x] Strip model + renderer (`statline-ui`) with measured layout
- [x] Strip window creation, cross-process `SetParent`, positioning math,
      1 s timer, `ArcSwap` snapshot read, `UpdateLayeredWindowIndirect` present
- [ ] **Strip pixels are not yet visible in the taskbar** (see below)
- [ ] M3 remainder: visibility rules (fullscreen/auto-hide), fallback tiers, hover
- [ ] M4–M9

## Immediate next step (start here)

The strip window attaches (`taskbar strip attached` in the log) and the renderer
produces correct pixels in isolation, but nothing appears over the taskbar. Investigate
in this order:

1. Confirm the surface has content at present time: log `surface.pixels_rgba()` alpha
   sum once, right before `present_layered`. If zero, `render()`/`TextEngine` is not
   drawing into the bound DIB.
2. Check `UpdateLayeredWindowIndirect` return value. It is currently ignored. With a
   **child** window, `pptDst` semantics differ from top-level windows: try
   `pptDst = null` (window position, current code) **and** explicit screen coordinates,
   and try presenting a plain `WS_EX_LAYERED` top-level window first (no `SetParent`)
   to isolate parenting from presentation.
3. If a child layered window still refuses, switch presentation to DirectComposition
   (DComp visual on the child HWND) — the path TrafficMonitor uses on Win11 — while
   keeping the same D2D drawing code.
4. Verify z-order: after `SetParent`, call `SetWindowPos(hwnd, HWND_TOP, ...)` within
   the taskbar to ensure the strip is above the XAML island.

Relevant files: `crates/statline-render/src/surface.rs` (`present_layered`),
`crates/statline-win/src/strip_window.rs`, `crates/statline-app/src/strip_host.rs`.

## Known cleanup

- `statline-render` warnings: unused `TextStyle` import in `tokens.rs`, unused `target`
  field in `Surface`, unused `WS_EX_*` imports in `strip_host.rs`, one unsafe call
  needing an explicit block in `strip_proc`. All are mechanical.
- `Surface::present_layered` takes `(x, y)` but ignores them now; decide the contract
  (window-position vs explicit destination) as part of the fix above.

## Measured facts

Ryzen 7 4700U, 7.4 GiB, Radeon iGPU · Windows 11 25H2 build 26220.9568 · 1536x864
logical @125 % (taskbar 48 logical / 60 physical px) · taskbar left-aligned, tray at
right · `MSTaskSwWClass` absent (XAML taskbar) · `CopyFromScreen` misses the taskbar,
`PrintWindow` captures it.

Sampling (release): cpu mean 0.78 ms/sample at 1 Hz = 0.078 % of one core, WS 14.3 MB.
App with strip + renderer attached: WS 17.2 MB.

## Log

- 2026-10-24 — M0: scaffold, tray, docs, tooling.
- 2026-10-24 — M1: all providers + sampler thread (3 commits).
- 2026-10-24 — M2: renderer foundation, tokens, text, PNG snapshots (verified).
- 2026-10-24 — M3 partial: strip window lifecycle wired; pixels not visible yet.
