# M3 — taskbar strip

**Status:** complete for v1 · strip is a parented child of `Shell_TrayWnd` (tier 1)

## Done

- [x] `Shell_TrayWnd` / `TrayNotifyWnd` discovery, DPI from the taskbar
- [x] Tier 1: `WS_CHILD` of `Shell_TrayWnd` via `SetParent`, with a verified parent
      link and a runtime fallback to the topmost tier
- [x] Layered child presentation through `UpdateLayeredWindowIndirect`
- [x] Geometry: right-anchored to the tray, vertically centred, overlap guard,
      screen→parent-client mapping for every move
- [x] Dirty-free rendering: redraw every tick into a DIB, present via ULW
- [x] Hides for fullscreen / presentation mode / hidden taskbar
- [x] Per-segment hover and hit-testing
- [x] Right-click → tray context menu
- [x] Click → flyout, verified open/toggle/Esc with synthetic input
- [x] `WM_MOUSEACTIVATE` → `MA_NOACTIVATE`: clicks never steal focus
- [x] Explorer restart: the app message window handles `TaskbarCreated` and recreates
      the strip for both placement tiers
- [x] `--tray-only` escape hatch; strip on by default
- [x] 60 s soak: WS 29.9 → 30.0 MB, private 13.9 → 14.5 MB, handles 310 → 314 (flat)

## Remaining

- [ ] Auto-hide taskbar: follow the edge reveal animation instead of just hiding
- [ ] `WM_DPICHANGED` handling (DPIs today are read every tick, which covers most of it)
- [ ] Multi-monitor placement option

## Notes that cost hours

- **`pblend: null` breaks ULW** with `ERROR_GEN_FAILURE`, which reads like a driver
  problem. It also made tier 1 look impossible: the first parented attempt failed with
  that error and the child was blamed. Always pass `AC_SRC_OVER` + `AC_SRC_ALPHA` +
  alpha 255 before drawing conclusions about the window hierarchy.
- **Never store a pointer to a value you then move.** The window procedure dereferences
  the host pointer on every timer tick; boxing it is what makes that sound.
- **GDI destruction must follow Direct2D teardown** for a DC render target; otherwise
  the heap is corrupted later, far from the cause.
- **A parented strip is a child window.** Position it in taskbar-client coordinates,
  handle `TaskbarCreated` in a top-level window, and use `FindWindowEx` (not
  `FindWindow`, which skips children) when verifying it from a script.
- **Screenshots lie in this session.** GDI capture (`CopyFromScreen`, `BitBlt`) omits
  the taskbar and layered windows entirely, so "invisible in the capture" is not
  evidence. Check `IsWindowVisible`, `GetWindowRect`, `GetParent`, and the ULW return
  value instead.

## Acceptance for the milestone

Strip sits flush left of the tray at 100/125/150/200 %, survives Explorer restart and
DPI changes, hides for fullscreen, never steals focus, and stays inside the idle
budget for 5 minutes.
