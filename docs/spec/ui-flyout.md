# Spec — flyout

A 360 × up-to-520 dip panel that opens above the strip. It is the "one more look"
surface: no settings, no navigation, everything scannable in one screen.

```
┌──────────────────────────────────────────┐
│ CPU                              23%     │  ← card: header + big value
│ ▁▂▃▅▆▇███▇▆▅▃▂▁ (60 s)                   │
│ 16 cores · 4.2 GHz                       │
│ ▮▮▮▯▮▮▯▮▮▮▮▯▮▮▮▮  per-core bars          │
├──────────────────────────────────────────┤
│ Memory                           62%     │
│ 9.8 / 16.0 GB · commit 11.2 / 19.0 GB    │
├──────────────────────────────────────────┤
│ GPU                              8%      │
│ 1.1 / 8.0 GB · 54 °C · 12 W              │
├──────────────────────────────────────────┤
│ Disk                             4%      │
│ C: 412 / 931 GB · R 12 MB/s W 3 MB/s     │
├──────────────────────────────────────────┤
│ Network                                  │
│ ↓ 1.24 MB/s   ↑ 84 KB/s                  │
├──────────────────────────────────────────┤
│ Top processes                            │
│ Code          4.2%   612 MB              │
│ Edge          1.8%   388 MB              │
│ …                                        │
├──────────────────────────────────────────┤
│ Dashboard                    Settings    │
└──────────────────────────────────────────┘
```

## Behaviour

- 12 dip corner radius, 1 dip border at 8 % text colour, soft shadow (drawn, not a DWM
  effect, so it works on a layered window).
- Opens anchored: `x = clamp(segment_center − width/2, screen.left + 8, screen.right − width − 8)`,
  `y = strip.top − height − 8`; if it would leave the monitor, open below.
- Closes on: focus loss, Esc, click outside, strip click again, display change.
- Scrolls with the mouse wheel when content exceeds the max height; no scrollbar chrome
  until hover.
- Updates live from the same snapshot; no separate sampling.

## Cards

Each card is optional and ordered by user preference (same order as the strip).

| Card | Content |
|---|---|
| CPU | 60 s area sparkline, total %, per-core mini bars, frequency |
| Memory | used/total, commit, cached, 60 s line |
| GPU | util %, VRAM, temp/fan/power when available, per-process GPU in the dashboard only |
| Disk | per-volume free/total bars, aggregate read/write, drive temp when available |
| Network | per-adapter rows: name, ↓/↑, session total |
| Battery | %, time left, rate, health, temp |
| System | uptime, processes, threads, handles, ACPI "System" temperature |
| Processes | top 5 by CPU with RAM; click a row → open its file location? no — copy path |

## Performance

- Rendered only when open; while open it redraws at most once per tick.
- Process table is sampled at 2 s while the flyout is open (5 s otherwise), and never
  when no process-displaying surface is visible.
- No animation on open beyond a 90 ms fade (skipped if "reduce motion" is on).
