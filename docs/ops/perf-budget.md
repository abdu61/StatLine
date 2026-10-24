# Ops — performance budget

A monitor that costs more than the thing it monitors is a bug.

## Targets

| Metric | Budget | Measured by |
|---|---|---|
| Sampling, 1 s interval | **mean < 1.5 ms** per sample (0.15 % of one core) | `--bench 60` |
| Sampling, p95 | < 3 ms (informational; includes scheduler noise on a busy desktop) | `--bench 60` |
| Idle CPU, strip visible | < 0.2 % of one core (avg over 5 min) | Task Manager, M3 onwards |
| Idle CPU, strip hidden | < 0.05 % | Task Manager |
| Working set, sampler only | < 30 MB | `--bench` |
| Working set, strip + flyout | < 45 MB | manual |
| GPU usage at idle | 0 % | Task Manager → GPU |
| Handles / threads | flat over 1 h (±2) | 1-hour soak |
| Startup → strip visible | < 250 ms | stopwatch in log |
| Release binary size | < 6 MB | `dir` |

`--bench` reports mean and p95. The pass/fail gate is the mean, because a single
1-second operation is dominated by scheduler jitter on a loaded machine; the mean over
60 samples is the number that maps to CPU share. `--profile` reports the tight-loop
cost of each provider, which is the right signal for optimisation.

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
| Volume enumeration | ~72–80 us |
| Full `Sampler::sample` | ~0.47 ms |

## Design consequences baked in

- `GetIfTable2` is far too expensive to call every tick. Interfaces are enumerated
  once per **60 s**; each tick reads counters per interface with `GetIfEntry2`.
- Volume capacity is refreshed every **30 s**, not every tick.
- PDH uses one cached query and English counter paths (localization-proof, stable cost).
- Process tables and GPU instance lists are only sampled while a surface displaying
  them is visible.

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
date | build | interval | mean ms | p95 ms | WS MB | private MB | notes
```
