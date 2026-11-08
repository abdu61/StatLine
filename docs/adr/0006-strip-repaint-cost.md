# ADR 0006 — the strip repaint costs 0.4 % of a core, not 0.2 %

Status: accepted, 2026-11-08.

## Context

`docs/ops/perf-budget.md` sets **idle CPU, strip visible < 0.2 % of one core**. The
figure in the perf log (0.46 ms per sample, 0.8 ms measured) came from `--bench`, which
times `Sampler::sample` in a tight loop. That is not what Task Manager shows.

Measured on this machine (Ryzen 7 7840U, 8 cores, Windows 11 26220, 125 % scaling,
1 s interval, no detail surface open), 20 samples at 15 s:

| Configuration | CPU, mean of one core |
|---|---|
| `--tray-only` (no strip) | **0.000 %** |
| `--safe-mode` (strip, documented counters only) | 0.243 % |
| strip live, all providers | **0.39–0.42 %** |

So the tray path is free, and the strip tick costs ~0.4 % regardless of how much
sampling does. Confirmed by setting the interval to 5000 ms: 0.406 % → 0.243 %. Fixed
cost per repaint, not per sample.

Instrumented per tick (`STATLINE_TRACE_TICK=1`): **2.3–3.6 ms** wall, of which
Direct2D text drawing is 1–4 ms and `present_layered` the rest.

## What was already fixed

- Every string was measured **twice** per tick (`measure` then `render`). `render` now
  takes the widths `measure` produced. First-seen win, small.
- `taskbar::primary()` was called every tick; it opens
  `HKCU\...\Explorer\Advanced` for `TaskbarAl` and walks taskbar children. The host now
  re-reads geometry only when the taskbar rect changes or 500 ms have passed.
- `SHQueryUserNotificationState` also ran every tick; now cached for 500 ms.
- Text formats were already cached; a width cache was added.

Measured after: unchanged at ~0.39 %. **The remaining cost is the Direct2D draws
themselves**, not bookkeeping.

## Decision

Keep the strip, accept ~0.4 %, and correct the budget table to the measured number
rather than leave a line that only ever passed because it measured the wrong thing.

## Why not cut the cost

The alternatives, and why each was rejected:

1. **Repaint only when a value changes.** Already implemented — but the values change
   every tick by design. A CPU percentage with 1 % resolution changes most seconds.
2. **Repaint less often than the sample interval.** Makes the strip lag its own data; the
   product is a live monitor.
3. **Cache glyph runs / use a sprite atlas.** Large change to the renderer for ~0.2 %.
4. **GDI instead of Direct2D.** Rejected in ADR 0003: the text and shape quality is what
   the product is judged on.
5. **Sample and draw at 2 s.** Default interval is a user setting; 2 s would be a
   regression in what the app is for.

0.4 % of one core on an 8-core machine is **0.05 % of total capacity**, and the strip is
only visible when the user asked for it. The tray-only path genuinely is 0.000 %.

## Consequences

- `perf-budget.md` now states the measured figures and distinguishes "sample cost"
  (what `--bench` measures) from "visible cost" (what Task Manager shows).
- The 0.2 % line remains as a **target**, explicitly not met, with this ADR as the reason
  rather than a silent edit.
- Any future change that adds work to the tick must show its cost here.
