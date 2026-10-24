# Spec — dashboard & settings window

One window, 960 × 640 dip (minimum 760 × 520), resizable, remembered position.

```
┌────────────┬───────────────────────────────────────────────┐
│  Statline  │  Overview                                     │
│            │  ┌─────────┐ ┌─────────┐ ┌─────────┐          │
│  Overview  │  │ CPU 23% │ │ MEM 62% │ │ GPU  8% │  cards   │
│  Modules   │  └─────────┘ └─────────┘ └─────────┘          │
│  Appearance│  ┌───────────────────────────────────────────┐ │
│  Behaviour │  │  5-minute chart, hover for a value        │ │
│  About     │  └───────────────────────────────────────────┘ │
│            │  Top processes  (sortable table)              │
└────────────┴───────────────────────────────────────────────┘
```

## Sections

**Overview** — live cards for enabled modules, one 5-minute chart (70 samples), and a
sortable process table (CPU / memory / name). Chart hover shows the value and time.

**Modules** — drag to reorder; toggle per module; per-module options (colour override,
meter style, warning/critical thresholds). Reorder order is shared with the strip and
flyout.

**Appearance** — theme (System / Light / Dark), accent (System / custom), strip
placement (left / right), density (comfortable / compact), show labels, meter style,
strip opacity, warning/critical colours.

**Behaviour** — sampling interval (0.5 / 1 / 2 / 5 s), start with Windows, show on
secondary monitors, hide in fullscreen, pause on battery saver, double-click action,
"safe mode" (forces the topmost/tray fallback).

**About** — version + commit, active taskbar tier, Windows build, log folder (opens),
data folder (opens), copy diagnostics, reset settings, licence, third-party notices,
and one line explaining why CPU temperature needs a sensor backend.

## Interaction & keyboard

- Tab / Shift+Tab through controls; Space toggles; arrows adjust sliders; Esc closes.
- Every setting applies live unless marked "restart required".
- Changes are debounced (300 ms) before writing to disk; the window never blocks on I/O.
- Closing the window hides it; the app keeps running in the tray.

## Visual rules

- DWM rounded corners + system border; custom title area drawn by us (no XAML).
- Content background drawn from the palette; optional Mica is a stretch goal.
- 12 dip card radius, 1 dip separators, 16 dip gutters, 24 dip page padding.
- No scrollbars unless needed; sections scroll independently only if the window is
  shorter than the minimum.

## Performance

- The dashboard samples only what it displays; when closed, process sampling stops.
- Charts are ring buffers; no disk history in v1.
- Window open cost target: < 16 MB additional working set, released on close.
