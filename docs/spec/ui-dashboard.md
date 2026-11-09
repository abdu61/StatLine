# Spec — companion window

One framed window, 880 × 680 dip, resizable. The renderer fills the client area.

The nav carries a LIVE chip, the page title sits above a short accent rule, cards are
outlined, and the active nav item gets an accent marker.

```
┌────────────┬───────────────────────────────────────────────┐
│  StatLine  │  Dashboard                                    │
│            │  ┌─────────────────┐ ┌─────────────────┐      │
│  Dashboard │  │ CPU 23%  ▁▂▃▂▅  │ │ MEM 62%  ▅▅▆▇█  │      │
│  CPU       │  └─────────────────┘ └─────────────────┘      │
│  Memory    │  ┌─────────────────┐ ┌─────────────────┐      │
│  GPU       │  │ GPU  8%         │ │ Disk 4%         │      │
│  Disk      │  └─────────────────┘ └─────────────────┘      │
│  Network   │  ... Battery, System cards                     │
│  Battery   │  Top processes (two columns)                   │
│  System    │                                                │
│  Settings  │                                                │
└────────────┴───────────────────────────────────────────────┘
```

## Navigation

The left column is a fixed 168 dip nav. Items: Dashboard, CPU, Memory, GPU, Disk,
Network, Battery, System, Settings. Clicking an item switches the page; Settings opens
the separate settings window. The selected item gets a 4 dip rounded background and the
accent colour.

- **Dashboard** — all module cards in a two-column grid, then the top-process table.
- **CPU** — CPU card with a 60 s chart, per-core bars, frequency, then Battery and
  System cards and the CPU process table.
- **Memory** — memory card (used/total, commit), cached row, top processes by working
  set, plus Battery and System.
- **GPU** — utilization, VRAM, and vendor temperature/fan/power when available.
- **Disk** — capacity/rate card, then per-volume used/total and temperature rows.
- **Network** — compact down/up card, then per-adapter rates and session totals.
- **Battery / System** — the two always-on cards, ordered for the page. The System page
  adds the Windows edition/build, computer name and process count.
- **About** — version, privacy statement, settings/log folders, licence, and what the
  sensor stack does and does not provide.

Module pages are built from the same `flyout::Model` focused on the page's module, so
the flyout and companion never disagree.

## Visual rules

- DWM rounded corners and system border; immersive dark mode follows the theme.
- Optional **Mica window effect** (`appearance.mica`): `DWMWA_SYSTEMBACKDROP_TYPE = 2`
  on the framed companion and settings windows. The layered flyout cannot host a DWM
  backdrop; it keeps its opaque panel (true per-pixel blur needs a composition
  swapchain and is out of scope for v1).
- Cards: 8 dip radius, 16 dip gutters, 24 dip page padding.
- No scrollbars; the design height fits the default content. Resizing smaller clips.

## Interaction & keyboard

- Click a nav item to switch pages; Esc closes the window.
- Settings open from the nav or the tray menu; changes apply live.
- Closing the window hides it; the app keeps running in the tray.

## Performance

- The companion samples only what it displays; process and sensor providers stop when
  the window closes.
- Charts are in-memory ring buffers; no disk history in v1.
- Open cost measured at ~39 MB WS / ~18 MB private with the process table active, flat
  across open/close cycles.
