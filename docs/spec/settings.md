# Spec — settings

Source of truth for `%APPDATA%\Statline\settings.json`. Rust types live in
`statline-config`; this file is the contract.

```jsonc
{
  "schema_version": 1,

  "modules": {
    "order": ["cpu", "memory", "gpu", "disk", "network"],
    "enabled": { "cpu": true, "memory": true, "gpu": true, "disk": true,
                 "network": true, "battery": true, "system": false },
    "options": {
      "cpu":     { "meter": "bar", "warn": 85, "critical": 95, "color": null },
      "memory":  { "meter": "bar", "warn": 85, "critical": 95, "color": null },
      "gpu":     { "meter": "bar", "warn": 90, "critical": 98, "color": null },
      "disk":    { "meter": "bar", "warn": 90, "critical": 98, "color": null },
      "network": { "meter": "none", "warn": 0, "critical": 0, "color": null }
    }
  },

  "appearance": {
    "theme": "system",            // system | light | dark
    "accent": "system",           // system | "#RRGGBB"
    "density": "comfortable",     // comfortable | compact
    "show_labels": true,
    "strip_placement": "right",   // right | left
    "strip_opacity": 1.0,         // 0.4 – 1.0
    "use_tabular_figures": true
  },

  "behaviour": {
    "interval_ms": 1000,          // 500 | 1000 | 2000 | 5000
    "start_with_windows": true,
    "hide_in_fullscreen": true,
    "show_on_secondary": false,
    "pause_on_battery_saver": false,
    "double_click": "dashboard",  // dashboard | flyout | none
    "safe_mode": false,
    "units": "binary"             // binary (KiB) | decimal (kB)
  },

  "window": {
    "dashboard": { "x": null, "y": null, "width": 960, "height": 640 }
  },

  "telemetry": { "enabled": false }   // exists only so it can never be turned on silently
}
```

## Rules

- Unknown keys at any level are preserved on save (forward compatibility).
- Missing keys fall back to defaults; no settings file is created until a setting
  actually changes.
- Invalid values are clamped and logged, never rejected wholesale.
- `telemetry.enabled` is read-only in the UI (always false in v1) and is documented in
  the README. If that ever changes, it needs an ADR and a privacy-policy update.
- Every save is atomic; failures are logged and surfaced once in the UI, not in a loop.
