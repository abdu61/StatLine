# Status

**Milestone:** M3 in progress (strip live) · **Updated:** 2026-10-24

## Headline

The strip renders and presents on the taskbar. `UpdateLayeredWindowIndirect` returns
`true`, the window is visible/topmost/layered at the correct rect, and the process runs
for minutes without crashing.

```
strip 298x50 at (1287,1025) presented=True
```

The last three bugs were all mine, not Windows':

1. **Null `BLENDFUNCTION`** in `UPDATELAYEREDWINDOWINFO` → the call fails with the very
   misleading `ERROR_GEN_FAILURE (0x8007001F)`. Passing `AC_SRC_OVER` + `AC_SRC_ALPHA`
   fixed presentation. The earlier "this VM cannot do layered windows" theory was
   wrong and has been removed from the ADR.
2. **Dangling pointer**: `Host::start` stored `&mut host` then returned the value by
   move. The timer fired into the old stack slot → access violation / heap corruption.
   The host is now boxed so its address is stable.
3. **Destruction order**: the GDI DC/bitmap were deleted before Direct2D released the
   render target bound to them. `GdiSurface` now drops after the D2D fields.

## Now

- [x] Strip renders: CPU / MEM / GPU / DISK / NET with meters, measured layout
- [x] Presentation via ULW with per-pixel alpha
- [x] Positions itself left of the tray, vertically centred, DPI-scaled (125 %)
- [x] Hides for fullscreen apps / presentation mode / hidden taskbar
- [x] Re-asserts topmost each tick; runs by default (`--tray-only` opts out)
- [ ] Visual sign-off from the user (screenshots cannot capture composited surfaces
      in this session — GDI capture omits both the taskbar and layered windows)
- [ ] Hover state, per-segment hit-testing, right-click menu on the strip
- [ ] Auto-hide taskbar edge behaviour (currently hides with the whole taskbar)

## Next, in order

1. Visual sign-off, then adjust spacing/typography from the screenshot.
2. Strip interaction: hover tint per segment, click → flyout anchor (M5), right-click →
   the tray menu.
3. M4: single instance (mutex + hand-off), autostart (HKCU Run), log rotation, panic
   hook, `--settings` routing.
4. M5: flyout panel.

## Performance (measured, release)

| Mode | Working set | Private | Handles | Threads |
|---|---|---|---|---|
| `--tray-only` | 10.6 MB | 1.8 MB | 145 | 4 |
| strip + sampler (steady) | **30.2 MB** | 14.2 MB | 310 | 9–12 |
| 60 s soak | 29.7 → 30.2 (plateau) | 14.0 → 14.2 | 309 → 310 | stable |

Sampling cost: cpu mean 0.78 ms/sample at 1 Hz = **0.078 % of one core**.

Direct2D + DirectWrite account for ~19 MB of working set and ~12 MB private; that is
the price of the drawing stack. Recorded in `docs/ops/perf-budget.md` with a revised
working-set target.

## Log

- 2026-10-24 — M0: scaffold, tray, docs, tooling.
- 2026-10-24 — M1: providers + sampler thread.
- 2026-10-24 — M2: renderer, tokens, text, PNG snapshots.
- 2026-10-24 — M3: strip live on the taskbar; presentation, dangling-pointer and
  destruction-order bugs fixed; visibility rules added.
