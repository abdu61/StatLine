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
