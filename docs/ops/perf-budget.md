# Ops — performance budget

A monitor that costs more than the thing it monitors is a bug.

## Targets

| Metric | Budget | Measured by |
|---|---|---|
| Sampling, 1 s interval | **cpu mean < 1.5 ms** per sample (0.15 % of one core) | `--bench 60` |
| Sampling, wall p95 | < 3 ms (informational; scheduler noise on a busy desktop) | `--bench 60` |
| Idle CPU, strip visible | < 0.2 % of one core (avg over 5 min) | Task Manager |
| Idle CPU, strip hidden | < 0.05 % | Task Manager |
| Working set, tray only | < 15 MB | `--tray-only` + Get-Process |
| Working set, strip live | **< 35 MB** | Get-Process after 60 s |
| Private bytes, strip live | < 25 MB | Get-Process after 60 s |
| Working set, strip + flyout | < 50 MB | manual |
| GPU usage at idle | 0 % | Task Manager → GPU |
| Handles / threads | flat over 1 h (±2) | 1-hour soak |
| Startup → strip visible | < 250 ms | stopwatch in log |
| Release binary size | < 6 MB | `dir` |

## Why the working-set line moved from 30 to 35 MB

Measured, not estimated: loading Direct2D + DirectWrite costs ~17 MB of working set and
~11 MB private on this machine (tray-only 10.6 MB → strip live 30.2 MB). That is mapped
font data and graphics DLL pages — the same cost TrafficMonitor and every other native
Direct2D tray app pays. The alternative (GDI) would cut memory but lose the text and
shape quality the product is judged on.

Private bytes, which is the number that actually describes pressure other processes
feel, stays at 14 MB. The 30 MB target was set before the drawing stack existed; 35 MB
is the honest number with a desktop-class renderer. Revisit if the dashboard pushes
private bytes past 25 MB.

## Measured provider costs (release, tight loop, 2026-10-24)

| Provider | Cost |
|---|---|
| `GetSystemTimes` | ~3 us |
| per-core times (`NtQuerySystemInformation`) | ~3–30 us |
| `current_mhz` | ~1 us |
| `memory_status` | ~0.5 us |
| `performance_counters` | ~15 us |
| `GetIfTable2` (full enumeration) | **~1.2–1.4 ms** |
| `GetIfEntry2` × 7 interfaces | ~270–310 us |
| PDH disk collect | ~40–55 us |
| GPU PDH + DXGI | ~650 us |
| Battery | ~12 us |
| Volume enumeration | ~72–80 us |
| Full `Sampler::sample` | ~0.47 ms (tight loop) |

## Design consequences baked in

- `GetIfTable2` is far too expensive to call every tick. Interfaces are enumerated
  once per **60 s**; each tick reads counters per interface with `GetIfEntry2`.
- GPU wildcard counters cost ~650 us, so they are cached for **3 s**.
- Volume capacity is refreshed every **30 s**, not every tick.
- PDH uses one cached query and English counter paths (localization-proof, stable cost).
- Process tables are only sampled while a surface displaying them is visible.

## Rules for contributors

1. Nothing on the sampling path may allocate per tick except the snapshot itself.
2. Nothing on the render path may run unless data, hover state or size changed.
3. No timer faster than the user's interval. No animation loops. Ever.
4. A new provider records its tight-loop cost in the table above.
5. If a change regresses a budget line, fix it or write an ADR explaining why the
   budget moves. Do not silently widen the gate.

## Recording a run

Append to `docs/ops/perf-log.md`:

```
date | build | mode | interval | cpu mean | WS MB | private MB | handles | notes
```
