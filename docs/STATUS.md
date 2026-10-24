# Status

**Milestone:** M1 + M2 complete · M3 blocked on presentation · **Updated:** 2026-10-24

## What works

- **M0** — workspace, tray shell, logger, settings, docs index, capture tooling.
- **M1** — CPU (total/per-core/MHz), memory/commit/cache, disk (PDH), network
  (per-adapter), GPU (util + VRAM), battery; sampler thread with `ArcSwap` publish.
  Measured: **0.78 ms CPU per sample at 1 Hz = 0.078 % of one core**, WS 14.3 MB.
- **M2** — software Direct2D surface (DIB + DC target), palette/type tokens, DirectWrite
  text engine, dependency-free PNG writer. Verified visually: the smoke test renders
  crisp text, rounded shapes and meters into a real PNG.
- **Strip model + drawing** — `statline-ui` measures and draws a 298x50 strip with
  CPU/MEM/GPU/DISK/NET segments; verified that it produces 314k alpha pixels.

## The blocker (read this first)

`UpdateLayeredWindowIndirect` fails with **ERROR_GEN_FAILURE (0x8007001F)** on this
machine. What has been ruled out:

| Test | Result |
|---|---|
| Child window parented to `Shell_TrayWnd` | ULW fails |
| Top-level `WS_POPUP` + `WS_EX_LAYERED` + `WS_EX_TOPMOST` | ULW fails |
| DIB filled **directly**, bypassing Direct2D entirely | **ULW still fails** |

So it is not Direct2D, not the render target, and not parenting. The same environment
also fails to include the taskbar in `CopyFromScreen` captures, which points at a
virtual/remote display driver without full composited-window support. **Layered-window
presentation is very likely unavailable on this VM.**

## Next steps, in order

1. **Implement presentation with `SetLayeredWindowAttributes(hwnd, key, 255, LWA_COLORKEY)` + `BitBlt` from the DIB into the window DC.** This is TrafficMonitor's
   `RenderType::DEFAULT` fallback and does not need ULW. Colour-key means no per-pixel
   alpha for the strip background — use a key colour and accept slightly harder edges,
   or draw the strip on the taskbar's own colour.
2. **Or** switch to DirectComposition (visual + `IDCompositionDevice` on the HWND),
   which is what TrafficMonitor uses for Win11 and is the path to proper per-pixel
   alpha.
3. Verify on a physical Windows 11 machine before concluding anything about the design.
   The renderer and geometry are already proven.

Relevant files: `crates/statline-render/src/surface.rs` (`present_layered`),
`crates/statline-win/src/strip_window.rs`, `crates/statline-app/src/strip_host.rs`.

**The strip is behind `--strip` until presentation works.** Without the flag the app
runs the tray path only and is stable (verified 4 s run).

## Known issue to fix with presentation

A run with `--strip` once exited with `STATUS_HEAP_CORRUPTION` (0xC0000374) after a
failed present. The diagnostic `pixels_rgba()` call that ran at that moment has been
removed. Re-test after the presentation change; if it recurs, suspect the DIB lifetime
vs. the failed `UpdateLayeredWindowIndirect` call.

## Measured facts (this machine)

Ryzen 7 4700U, 7.4 GiB, Radeon iGPU · Windows 11 25H2 build 26220.9568 · 1536x864
logical @125 % · taskbar 48 logical / 60 physical px, left-aligned · tray left edge at
x=1590 physical · `MSTaskSwWClass` absent (XAML taskbar).

Sampling: cpu mean 0.78 ms/sample · app with strip + renderer: WS 17.2 MB.

## Log

- 2026-10-24 — M0: scaffold, tray, docs, tooling.
- 2026-10-24 — M1: all providers + sampler thread (3 commits).
- 2026-10-24 — M2: renderer foundation, tokens, text, PNG snapshots (verified).
- 2026-10-24 — M3 partial: strip renders and positions; ULW presentation blocked by the
  environment (see above); strip gated behind `--strip`.
