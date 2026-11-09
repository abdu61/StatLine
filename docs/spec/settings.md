# Spec — settings

Source of truth for `%APPDATA%\StatLine\settings.json`. Rust types live in
`statline-config`; this file is the contract.

```jsonc
{
  "schema_version": 1,

  "general": {
    "update_interval_ms": 1000,   // 500 | 1000 | 2000 | 5000
    "run_at_startup": false,      // per-user Run key for portable builds
    "strip_placement": "right",   // right | left
    "units": "binary"             // binary (KiB) | decimal (kB)
  },

  "appearance": {
    "theme": "system",            // system | light | dark
    "accent": "system",           // system | blue | green | purple | orange | teal | pink
    "mica": false                 // DWM Mica backdrop on framed windows
  },

  "modules": {
    "cpu": true, "memory": true, "gpu": true, "disk": true, "network": true
  }
}
```

## Rules

- Unknown keys at any level are **ignored** on load, so a newer build can write a
  document an older build still opens. They are not preserved on save.
- Missing keys fall back to defaults; no settings file is created until a setting
  actually changes.
- Invalid values are clamped (`update_interval_ms` snaps to a supported option) and
  never reject the whole document.
- Every save is atomic (temp file + rename); failures are logged and the window keeps
  the old value rather than showing a change that did not persist.
- `run_at_startup` mirrors the registry: the settings window reads the current `Run`
  value on open and treats it as the truth, and every toggle writes the registry before
  updating the document.

## Window state

The companion's outer rectangle is stored separately in
`%APPDATA%\StatLine\window.json` (physical pixels, `{x, y, width, height}`). It is
written on `WM_EXITSIZEMOVE` and on close, and only restored when the rectangle still
intersects a monitor and is at least 480×360. Minimized rectangles are never saved.
Keeping it out of `settings.json` means a drag does not race the settings window's
whole-document saves.

## Planned (not in schema v1 yet)

`modules` ordering and per-module meter/threshold/colour overrides,
`behaviour.hide_in_fullscreen`, `appearance.density`, `window.settings` bounds, and the
read-only `telemetry` block. Adding any of them is a schema change with a migration path.

## Where it is edited

Tray icon or strip right-click → **Settings…**. Rows cycle on click:

| Row | Values |
|---|---|
| Theme | System → Dark → Light |
| Update interval | 500 → 1000 → 2000 → 5000 ms |
| Units | Binary (KiB) ↔ Decimal (kB) |
| Accent | System → Blue → Green → Purple → Orange → Teal → Pink |
| Mica window effect | On ↔ Off (`DWMWA_SYSTEMBACKDROP_TYPE` on the framed windows) |
| Strip placement | Right ↔ Left (Left is the leftmost free area: far-left on a centred taskbar, after the app-button group on a left-aligned one) |
| Start with Windows | On ↔ Off (writes `HKCU\...\Run`) |
| Strip CPU / memory / GPU / disk / network | On ↔ Off (the flyout and companion stay complete) |

Changes apply live: the strip palette, sampler cadence, byte units and anchor update on
the next tick, and an open flyout/companion repaints with the new palette and units.
Binary is the default (KiB/MiB); decimal switches every surface to kB/MB.

Mica applies to the framed companion and settings windows. The strip and flyout are
layered windows and cannot host a DWM backdrop; the flyout instead becomes lightly
translucent when Mica is on. Turning Mica on shows a warning that DWM blur can cost GPU
power and battery.
