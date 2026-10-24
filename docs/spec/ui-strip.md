# Spec — taskbar strip

The strip is the product. Everything else is optional depth.

## Anatomy

```
┌──────────────┬────────┬────────┬────────┬────────┐
│  ↓ ↑  NET    │ ◉ CPU  │ ▤ MEM  │ ▦ GPU  │ ⛁ DISK │   ~40 dip tall
│ 1.2M / 84K   │  23%   │  62%   │  8%    │  4%    │
│              │ ▁▂▃▂▅  │ ▅▅▆▇█  │ ▁▁▂▁▁  │ ▁▁▁▂▁  │   meter strip, 2 dip
└──────────────┴────────┴────────┴────────┴────────┘
```

- Network is **first** on purpose. The strip is right-anchored to the tray, so the only
  variable-width segment changes the strip's left edge; CPU/MEM/GPU/DISK keep their
  screen positions instead of shifting sideways when the rate text changes.
- Height: `taskbar_height − 8 dip`, vertically centred.
- Horizontal padding: 6 dip per segment; 2 dip gap between segments.
- Minimum segment width: 44 dip. Width grows with the measured text.
- Corner radius 4 dip on hover only (no permanent chrome — it must look native).

## Typography (Segoe UI Variable, DirectWrite)

| Element | Size / weight | Colour |
|---|---|---|
| Label ("CPU") | 9 dip, semibold, +60 tracking (caps) | muted (60 % of text) |
| Value ("23%") | 12 dip, semibold, tabular figures | primary text |
| Unit ("/s") | 9 dip, regular | muted |
| Net row | 11 dip semibold for ↓/↑ values | primary |

Numeric values use tabular figures so digits do not jitter between frames.

## Colour

- Dark and light palettes derived from the Windows theme (`AppsUseLightTheme`).
- Accent from the system accent colour, with a per-module override.
- CPU / GPU / MEM meters: accent by default; DISK and NET use the same accent at 70 %
  opacity so the strip reads as one object, not five confetti colours.
- Warning thresholds: ≥ 85 % switches the value to the warning colour; ≥ 95 % to the
  critical colour. Thresholds configurable, colours from the palette.

## Meter styles (`appearance.meter_style`)

| Style | Rendering |
|---|---|
| `bar` (default) | 2 dip tall, full width of the segment, track at 12 % text colour |
| `spark` | 16 sample sparkline, 1 dip stroke + 6 % fill |
| `none` | text only, most compact |

## States

| State | Behaviour |
|---|---|
| Hover | segment background at 8 % text colour, 4 dip radius |
| Pressed | 12 % |
| Unavailable module | row omitted entirely (never a zero or a dash) |
| Paused | strip keeps last values at 40 % opacity, tooltip says "Paused" |
| Drain mode (`critical` threshold crossed) | value + meter animate once to the critical colour, then hold |

## Interaction

- Left click on a segment → the flyout opens focused on that module: only its card,
  its detail rows (per-core bars, volumes, adapters), plus System and Battery. CPU and
  Memory also show the matching process table.
- Left click on the strip background → flyout opens with every module.
- Right click → context menu: Pause / Resume · Placement ▸ Left / Right · Modules ▸ ·
  Settings · Dashboard · About · Exit.
- Double click → dashboard.
- Mouse wheel over the strip → adjust sampling interval? **No.** Accidental scrolling
  while using the taskbar must not change behaviour.

## Sizing rules

Measured, never assumed:

```
segment_width = padding*2 + max(label_width, value_width + unit_width, meter_min)
strip_width   = sum(segment_width) + gaps
```

If `strip_width` exceeds available space, degrade in this order: hide labels → hide
meters → hide DISK → hide NET → tray-only.

## Accessibility

- The strip is not focusable; the tray menu is the keyboard path (Windows+B).
- Tooltip text on hover exposes full values for screen readers.
- Minimum contrast for text ≥ 4.5:1 against the taskbar in both themes.
