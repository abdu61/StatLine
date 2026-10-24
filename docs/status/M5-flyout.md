# M5 — flyout

**Status:** in progress · opens, closes and shows the live process table

## Goal

One glance gives the whole picture, without opening a window.

## Checklist

- [x] Layered popup window, 12 dip radius, drawn shadow, anchored above the strip
- [x] A segment click shows only that module, plus System and Battery
- [x] CPU detail: per-core bars, frequency, CPU process table
- [x] Memory detail: used/total, commit, cache, top memory apps
- [x] Disk/Network detail: per-volume and per-adapter rows
- [x] Re-anchors when the process table makes the panel taller; screen-clamped
- [x] Open/close rules: focus loss, Esc, click-outside, strip click again
- [x] Only samples processes while visible (`set_process_table` gate)
- [ ] Overview click (strip padding) needs a deliberate hit target; the strip is fully
      covered by segments today
- [ ] Footer actions: Dashboard, Settings
- [ ] Wheel scrolling with no permanent scrollbar chrome
- [ ] Reduce-motion respected (no fade)
- [ ] Close on display change

## Exit criteria

Anchoring is correct on both edges and both placements; content updates live; opening
the flyout costs < 8 MB working set and returns to baseline on close.
