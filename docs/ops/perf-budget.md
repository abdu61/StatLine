# Ops — performance budget

A monitor that costs more than the thing it monitors is a bug.

## Targets

| Metric | Budget | Measured by |
|---|---|---|
| Sampling, 1 s interval | **cpu mean < 1.5 ms** per sample (0.15 % of one core) | `--bench 60` |
| Sampling, wall p95 | < 3 ms (informational; scheduler noise on a busy desktop) | `--bench 60` |
| Idle CPU, strip visible | target < 0.2 %, **actual 0.39 %** — see [ADR 0006](../adr/0006-strip-repaint-cost.md) | process CPU over 5 min |
| Idle CPU, tray only | < 0.05 % — measured 0.000 % | process CPU over 3 min |
| Working set, tray only | < 15 MB | `--tray-only` + Get-Process |
| Working set, strip live | **< 35 MB** | Get-Process after 60 s |
| Private bytes, strip live | < 25 MB | Get-Process after 60 s |
| Working set, strip + flyout | < 50 MB | manual |
| GPU usage at idle | 0 % | Task Manager → GPU |
| Handles / threads | flat over 1 h (±2) | 1-hour soak — measured 338–342 over 62.5 min |
| Startup → strip visible | < 250 ms | stopwatch in log |
| Release binary size | < 6 MB | `dir` — measured 0.68 MB |

## Two different numbers, and why the distinction matters

`--bench` times `Sampler::sample` in a tight loop. That is the **sampling** cost, and it
is ~0.4 ms. What Task Manager shows is the **visible** cost, which additionally includes
one repaint of the strip per tick.

Measured 2026-11-08 on this machine with no detail surface open:

| Configuration | CPU, mean of one core |
|---|---|
| `--tray-only` | 0.000 % |
| strip live, `--safe-mode` | 0.243 % |
| strip live, all providers | 0.39 % |

A 5000 ms interval changed that 0.406 % → 0.243 %, which is how the fixed per-repaint
cost was separated from the per-sample cost. The strip tick itself measures 2.3–3.6 ms
(`STATLINE_TRACE_TICK=1`), mostly Direct2D text drawing. See
[ADR 0006](../adr/0006-strip-repaint-cost.md) for what was optimised and what was
rejected.

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
| Process table (`NtQuerySystemInformation`) | **~4.75 ms** (250 processes) |
| ACPI thermal zone (PDH wildcard) | ~1.16 ms |
| Drive temperatures (2 IOCTLs per volume) | **~11.2 ms** |
| Battery details (SetupAPI + IOCTL) | ~0.14 ms |
| Page file (`SystemPageFileInformation`) | ~0.8 us |
| ADL PMLog / NVML (GPU vendor) | ~0.1–0.5 ms |
| WLAN SSID/signal (`wlanapi`) | **~12 ms** |
| Per-disk PDH (wildcard arrays) | ~36 us |
| Full `Sampler::sample` | ~0.38–0.46 ms (tight loop, detail providers off) |

## Design consequences baked in

- `GetIfTable2` is far too expensive to call every tick. Interfaces are enumerated
  once per **60 s**; each tick reads counters per interface with `GetIfEntry2`.
- GPU wildcard counters cost ~650 us, so they are cached for **3 s**.
- Volume capacity is refreshed every **30 s**, not every tick.
- PDH uses one cached query and English counter paths (localization-proof, stable cost).
- The process table costs ~4.75 ms per query, so it runs at 2 s and **only while the
  flyout or companion is open**; with every process surface closed it costs zero. That
  is ~0.24 % of one core while a process view is visible, and none at idle.
- Detail sensors share that visibility gate: ACPI zones (5 s, ~1.16 ms), battery health
  (30 s, ~0.14 ms), drive temperatures (5 min, ~11 ms) and WLAN (15 s, ~12 ms) do not run
  while only the strip is visible. This keeps the idle tight-loop sample at ~0.4 ms;
  opening the flyout raises the average to ~2.3 ms/sample because of the process table.
- GPU vendor telemetry rides the 3 s GPU refresh (NVML/ADL), not the tick.
- Page-file usage is a single `NtQuerySystemInformation` and runs with the memory sample.
- Drive temperatures are cached for five minutes because the volume and disk IOCTLs
  cost ~11 ms together; capacity alone stays on the 30 s refresh.

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
