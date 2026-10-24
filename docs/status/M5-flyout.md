# M5 — flyout

**Status:** not started

## Goal

One glance gives the whole picture, without opening a window.

## Checklist

- [ ] Layered popup window, 12 dip radius, drawn shadow, anchored above the strip
- [ ] Cards: CPU (sparkline + per-core), memory, GPU, disk, network, battery, system
- [ ] Top-process list (5 rows) with live CPU/RAM
- [ ] Footer actions: Dashboard, Settings
- [ ] Open/close rules: focus loss, Esc, click-outside, strip toggle, display change
- [ ] Wheel scrolling with no permanent scrollbar chrome
- [ ] Reduce-motion respected (no fade)
- [ ] Only samples processes while visible

## Exit criteria

Anchoring is correct on both edges and both placements; content updates live; opening
the flyout costs < 8 MB working set and returns to baseline on close.
