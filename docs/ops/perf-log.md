# Performance log

Append one line per measured run. Machine: see `environment.md`.

| Date | Build | Mode | Interval | cpu mean | WS MB | Private MB | Handles | Notes |
|---|---|---|---|---|---|---|---|---|
| 2026-10-24 | M0 | tray shell | — | — | 10.0 | 1.5 | 137 | no sampling |
| 2026-10-24 | M1a | bench | 1000 ms | 0.42 ms | 6.3 | 1.2 | — | CPU + memory only |
| 2026-10-24 | M1b | bench | 1000 ms | 1.43 ms | 12.4 | 5.4 | — | + disk + network |
| 2026-10-24 | M1c | bench | 1000 ms | 0.78 ms | 14.3 | 6.2 | — | + GPU + battery + thread |
| 2026-10-24 | M3 | `--tray-only` | — | — | 10.6 | 1.8 | 145 | no strip, no sampler |
| 2026-10-24 | M3 | strip live | 1000 ms | ~0.8 ms | 30.2 | 14.2 | 310 | steady state after 60 s |
| 2026-10-24 | M3b | strip live, parented child | 1000 ms | ~0.8 ms | 30.0 | 14.5 | 314 | tier 1: `WS_CHILD` of `Shell_TrayWnd` |
| 2026-10-24 | M6b | process provider, one query | — | 4.75 ms/query | — | — | — | 250 processes; 2 s cadence only while a process surface is open |
| 2026-10-24 | M6c | bench, process table off | 1000 ms | 0.78 ms/sample | 15.1 | 6.3 | — | per-sample CPU sum; total mean 1.04 ms (scheduler-quantized) |

## 90-second soak (M3b, parented strip)

```
 15s  WS=29.9  private=13.9  handles=314
 30s  WS=30.0  private=13.9  handles=314
 45s  WS=30.0  private=13.9  handles=314
 60s  WS=30.0  private=13.9  handles=314
 75s  WS=30.0  private=13.9  handles=314
 90s  WS=30.0  private=13.9  handles=310
```

Flat. The parent link costs one `GetParent` at startup and a `ScreenToClient` pair only
when the rect changes; there is no per-tick cost on the sampling path.

## Companion + process table (M6b, five open/close cycles)

```
 start     WS=29.7  private=13.7  handles=310
 open 1    WS=38.5  private=17.7  handles=374
 close 1   WS=35.9  private=15.0  handles=373
 open 5    WS=38.3  private=18.0  handles=374
 close 5   WS=35.7  private=15.3  handles=373
 idle +20s WS=35.7  private=15.3  handles=374
```

Flat from the first cycle onward: the +63 handles are the one-time Direct2D/DirectWrite
companion resources, not a leak. The process table adds ~0.5 MB private, and the query
is gated off on the first tick after the window closes.

## How `--bench` reports CPU

The OS charges CPU time in 15.6 ms scheduler ticks, so a 1 ms sample can show up as a
whole tick on the sample where the charge lands. `--bench` now prints `cpu/sample
min/mean/max` from per-sample deltas; the mean there (0.78 ms) is the real work, while
the coarse `cpu mean` line is an upper bound.

## 60-second soak (M3, strip live)

```
 10s  WS=29.7  private=14.0  handles=309  threads=10
 20s  WS=29.9  private=14.0  handles=309  threads=10
 30s  WS=30.2  private=14.3  handles=310  threads=12
 40s  WS=30.2  private=14.3  handles=310  threads=12
 50s  WS=30.2  private=14.3  handles=310  threads=12
 60s  WS=30.2  private=14.2  handles=310  threads=9
```

Flat. No leak in the sampler, the surface, or the text cache.

## Where the memory goes

| Component | WS | Private |
|---|---|---|
| Tray shell + sampler thread | 10.6 MB | 1.8 MB |
| Direct2D + DirectWrite loaded | +~17 MB | +~11 MB |
| Strip surface (298x50 DIB) + caches | +~2.6 MB | +~1.3 MB |

The D2D/DWrite delta is mostly mapped font data and graphics DLL pages; it is the price
of native text and shape rendering. See the revised budget in `perf-budget.md`.

## Commands

```powershell
cargo run --release -p statline-app -- --bench 60      # sampling gate
cargo run --release -p statline-app -- --profile 200   # per-provider cost
cargo run -p statline-app -- --probe                   # live values
```

## Note on measurement

`--bench` now reports process CPU time alongside wall time. Wall-clock per-sample
numbers on a loaded desktop are dominated by scheduler jitter (measured up to 3x the
real work), so the CPU figure is the gate.
