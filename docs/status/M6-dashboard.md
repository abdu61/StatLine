# M6 — dashboard & settings

**Status:** not started

## Goal

The place to configure and to explore, in one window.

## Checklist

- [ ] Window shell: custom title area, DWM rounded corners, resize + remembered bounds
- [ ] Left navigation: Overview / Modules / Appearance / Behaviour / About
- [ ] Overview: live cards, 5-minute chart with hover readout, sortable process table
- [ ] Modules: enable, reorder (drag), per-module meter/threshold/colour
- [ ] Appearance: theme, accent, placement, density, labels, opacity
- [ ] Behaviour: interval, autostart, secondary monitors, fullscreen, safe mode, units
- [ ] About: versions, active tier, folders, copy diagnostics, reset, licences
- [ ] Keyboard navigation + focus rings; Esc closes
- [ ] Live preview of strip changes while editing

## Exit criteria

Every setting in `spec/settings.md` is reachable, applies live and persists; the UI
never blocks on disk I/O; keyboard-only operation covers the whole window.
