# Status

**Milestone:** M3 complete (tier 1) · M4 complete · M5 verified · M6 partial ·
**Updated:** 2026-10-24

## Where it stands

The strip is a parented child of `Shell_TrayWnd`, so it stacks with shell surfaces
instead of fighting them. Flyout open/close, toggle and Esc were verified with synthetic
input against the parented strip. The companion window paints the full overview in a real
framed window.

```
strip hosted by Shell_TrayWnd (tier 1)
strip 237x42 at (1348,1028) presented=true tier=child
```

## Working

- **Strip**: CPU / MEM / GPU / DISK / NET with meters, hover plate per segment,
  right-click menu, parented into the taskbar (`WS_CHILD`), `WM_MOUSEACTIVATE`
  suppressed, hides for fullscreen and a hidden taskbar, recreates after an Explorer
  restart via the app message window. Topmost popup remains as an automatic fallback.
- **Flyout**: opens on strip click, toggles closed on a second click, closes on Esc or
  focus loss, renders interpolation-smoothed area charts.
- **Companion**: framed resizable Windows 11 window with a two-column card grid.
- **Sampling**: 0.78 ms per sample at 1 Hz (0.078 % of one core), WS 30.0 MB,
  private 14.5 MB, handles flat over a 90 s soak.
- **Lifecycle**: single instance, autostart commands, 1 MB × 3 log rotation, panic
  hook, `--tray-only`.

## Next, in order

1. **Process provider + per-app breakdown** (`NtQuerySystemInformation`), sampled at 2 s
   only while a process surface is visible. Click a segment → flyout focused on that
   module with its top processes; same table in the companion.
2. **Settings UI** (`statline-config` is written but unwired): theme, interval,
   placement, units, modules, autostart, safe mode; live-apply.
3. **Driver-free sensors (M7)**: GPU temp/fan/power, drive temps, battery health,
   ACPI thermal zone. Degrade to hidden; never fake a value.
4. **Packaging (M9)**: MSIX, portable zip, CI, README with snapshots, notices,
   local `v0.1.0` tag.
5. **Polish**: tooltips, keyboard path, empty states, `--safe-mode`, multi-monitor,
   `WM_DPICHANGED`, Mica on the companion.

## Measured

| Mode | WS | Private | Handles |
|---|---|---|---|
| `--tray-only` | 10.6 MB | 1.8 MB | 145 |
| strip live, parented (tier 1) | 30.0 MB | 14.5 MB | 314 |

D2D + DWrite account for ~19 MB of the working set; the budget is 35 MB with that
recorded in `docs/ops/perf-budget.md`. Handles stayed flat over the soak (314 → 310).

## Log

- M0 scaffold · M1 metrics · M2 renderer · M3 strip live, now parented into the taskbar ·
  M4 lifecycle · M5 flyout verified · M6 companion window.
