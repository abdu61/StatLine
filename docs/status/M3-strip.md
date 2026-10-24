# M3 — taskbar strip

**Status:** in progress · strip is live and stable

## Done

- [x] `Shell_TrayWnd` / `TrayNotifyWnd` discovery, DPI from the taskbar
- [x] Topmost layered popup (`WS_EX_LAYERED | NOACTIVATE | TOOLWINDOW`)
- [x] `UpdateLayeredWindowIndirect` with a real `BLENDFUNCTION`
- [x] Geometry: right-anchored to the tray, vertically centred, overlap guard
- [x] Dirty-free rendering: redraw every tick into a DIB, present via ULW
- [x] Hides for fullscreen / presentation mode / hidden taskbar
- [x] `--tray-only` escape hatch; strip on by default
- [x] 60 s soak: WS 29.7 → 30.2 MB, private 14.0 → 14.2 MB, handles 309 → 310

## Remaining

- [ ] Hover state per segment and hit-testing
- [ ] Right-click → tray context menu
- [ ] Click → flyout (needs M5)
- [ ] Auto-hide taskbar: follow the edge reveal animation instead of just hiding
- [ ] Explorer restart: destroy/recreate the strip window on `TaskbarCreated`
- [ ] `WM_DPICHANGED` handling (DPIs today are read every tick, which covers most of it)
- [ ] Multi-monitor placement option

## Notes that cost hours

- **`pblend: null` breaks ULW** with `ERROR_GEN_FAILURE`, which reads like a driver
  problem. Always pass `AC_SRC_OVER` + `AC_SRC_ALPHA` + alpha 255.
- **Never store a pointer to a value you then move.** The window procedure dereferences
  the host pointer on every timer tick; boxing it is what makes that sound.
- **GDI destruction must follow Direct2D teardown** for a DC render target; otherwise
  the heap is corrupted later, far from the cause.
- **Screenshots lie in this session.** GDI capture (`CopyFromScreen`, `BitBlt`) omits
  the taskbar and layered windows entirely, so "invisible in the capture" is not
  evidence. Check `IsWindowVisible`, `GetWindowRect`, `WS_EX_TOPMOST`, and the ULW
  return value instead.

## Acceptance for the milestone

Strip sits flush left of the tray at 100/125/150/200 %, survives Explorer restart and
DPI changes, hides for fullscreen, never steals focus, and stays inside the idle
budget for 5 minutes.
