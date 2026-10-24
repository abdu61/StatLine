# M5 — flyout

**Status:** in progress · opens, closes and shows the live process table

## Goal

One glance gives the whole picture, without opening a window.

## Checklist

- [x] Layered popup window, 12 dip radius, drawn shadow, anchored above the strip
- [x] Cards: CPU (sparkline + total), memory, GPU, disk, network, battery
- [x] Top-process list (5 rows) with live CPU/RAM, `—` until a CPU baseline exists
- [x] Open/close rules: focus loss, Esc, click-outside, strip click again
- [x] Only samples processes while visible (`set_process_table` gate)
- [ ] Per-core mini bars on the CPU card
- [ ] System card (uptime, threads, handles, ACPI "System" temperature)
- [ ] Footer actions: Dashboard, Settings
- [ ] Wheel scrolling with no permanent scrollbar chrome
- [ ] Reduce-motion respected (no fade)
- [ ] Close on display change

## Exit criteria

Anchoring is correct on both edges and both placements; content updates live; opening
the flyout costs < 8 MB working set and returns to baseline on close.
