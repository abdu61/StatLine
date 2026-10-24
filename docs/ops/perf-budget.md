# Ops — performance budget

A monitor that costs more than the thing it monitors is a bug.

## Targets

| Metric | Budget | Measured by |
|---|---|---|
| Idle CPU, strip visible, 1 s interval | < 0.2 % of one core (avg over 5 min) | `--bench 300` + Task Manager |
| Idle CPU, strip hidden | < 0.05 % | `--bench 300` (hidden) |
| Working set, strip only | < 30 MB | `--bench` reports it |
| Working set, dashboard open | < 45 MB | manual |
| Private bytes | < 45 MB | `--bench` |
| GPU usage at idle | 0 % | Task Manager → Performance → GPU |
| Handles / threads | flat over 1 h (±2) | `--bench 3600` |
| Per-sample wall time | < 1.5 ms p95 | `--bench` histogram |
| Startup → strip visible | < 250 ms | stopwatch in log |
| Release binary size | < 6 MB | `dir` on the exe |

## Rules for contributors

1. Nothing on the sampling path may allocate per tick except the snapshot.
2. Nothing on the render path may run unless data, hover state or size changed.
3. No timer faster than the user's interval. No animation loops. Ever.
4. A new provider must state its cost in this file when it lands.
5. If a change regresses a budget line, it does not merge — fix it or write an ADR
   explaining why the budget moves.

## Techniques already in use

- One waitable timer, one sampler thread, `ArcSwap` publish (no locks on the UI path).
- Software D2D rendering: no GPU device, no swapchain, no compositor animation.
- Process table and GPU instance lists sampled lazily, only while visible.
- `SetProcessInformation(ProcessPowerThrottling)` EcoQoS while hidden or fullscreen.
- No WMI, no ETW session, no polling of the registry.

## Recording a run

Append to `docs/ops/perf-log.md`:

```
date | build | interval | CPU % | WS MB | private MB | sample p95 | notes
```
