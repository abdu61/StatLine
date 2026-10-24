# Spec — settings

Source of truth for `%APPDATA%\Statline\settings.json`. Rust types live in
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
    "mica": false                 // DWM Mica backdrop on framed windows
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

## Planned (not in schema v1 yet)

`modules` (enable/order/options), `behaviour.hide_in_fullscreen`,
`appearance.density`, `window.dashboard` bounds, and the read-only `telemetry` block.
Adding any of them is a schema change with a migration path.

## Where it is edited

Tray icon or strip right-click → **Settings…**. Rows cycle on click:

| Row | Values |
|---|---|
| Theme | System → Dark → Light |
| Update interval | 500 → 1000 → 2000 → 5000 ms |
| Units | Binary (KiB) ↔ Decimal (kB) |
| Mica window effect | On ↔ Off (`DWMWA_SYSTEMBACKDROP_TYPE` on the framed windows) |
| Strip placement | Right ↔ Left (left anchors past Start, clamped at the tray) |
| Start with Windows | On ↔ Off (writes `HKCU\...\Run`) |

Changes apply live: the strip palette, sampler cadence, byte units and anchor update on
the next tick, and an open flyout/companion repaints with the new palette and units.
Binary is the default (KiB/MiB); decimal switches every surface to kB/MB.

Mica applies to the framed companion and settings windows. The strip and flyout are
layered windows and cannot host a DWM backdrop, so they keep their opaque panels.
