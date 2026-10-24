# Status

**Milestone:** M3 complete · M4 complete · M5 in progress · **Updated:** 2026-10-24

## Where it stands

The strip is live on the taskbar and stable, the tray menu drives pause and strip
visibility, lifecycle work (single instance, autostart, log rotation, panic hook) is
done, and the flyout renders correctly in snapshots.

```
strip 269x50 at (1316,1025) presented=True
```

## Working

- **Strip**: CPU / MEM / GPU / DISK / NET with meters, hover plate per segment,
  right-click menu, re-asserted topmost, hides for fullscreen and a hidden taskbar,
  `. . .` recreates after an Explorer restart.
- **Sampling**: 0.78 ms per sample at 1 Hz (0.078 % of one core), WS 30.2 MB,
  private 14.2 MB, flat over a 60 s soak.
- **Lifecycle**: single instance, autostart commands, 1 MB × 3 log rotation, panic
  hook, `--tray-only`.
- **Flyout rendering**: `--render-snapshots` produces the panel with interpolation-
  smoothed area charts; the design reads well at 125 %.

## Open issue (start here)

**The flyout window does not appear when the strip is clicked.** The handler runs (the
debug log `strip left click` fires) and no error is logged, so `Flyout::open` is
returning early or the window is created and immediately destroyed. Two suspects, in
order:

1. `create_popup_window` calls `SetForegroundWindow`, which makes the flyout active and
   then our `WM_ACTIVATE` handler fires with `WA_INACTIVE` when focus bounces back to
   the strip — closing it in the same message chain. Guard with a "just opened"
   timestamp, or close only on a *user* activation change.
2. `Flyout::open` is called with a borrow of the host while `paint` re-enters, and the
   early `return` in the error path leaves no trace — add a log at entry and after
   creation to confirm which branch runs.

## Next, in order

1. Fix the flyout open path, then verify `WM_ACTIVATE`/Esc dismissal.
2. Flyout polish: row dividers, disk/network sparklines, "Open dashboard" footer.
3. M6: dashboard and settings window (biggest remaining user-facing piece).
4. M7: driver-free sensors (GPU vendor temps, drive temps, ACPI zone).
5. M8: hover/keyboard polish, empty states, safe mode.
6. M9: MSIX + portable packaging, CI, README with the snapshot images.

## Measured

| Mode | WS | Private | Handles |
|---|---|---|---|
| `--tray-only` | 10.6 MB | 1.8 MB | 145 |
| strip live | 30.2 MB | 14.2 MB | 310 |

D2D + DWrite account for ~19 MB of the working set; the budget was revised to 35 MB
with that recorded in `docs/ops/perf-budget.md`.

## Log

- M0 scaffold · M1 metrics · M2 renderer · M3 strip live · M4 lifecycle · M5 flyout
  rendering (window open path still broken).
