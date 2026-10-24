# Performance log

Append one line per measured run. Machine: see `environment.md`.

| Date | Build | Interval | Mean ms | p95 ms | WS MB | Private MB | Notes |
|---|---|---|---|---|---|---|---|
| 2026-10-24 | 0.1.0 (M0) | — | — | — | 10.0 | 1.5 | tray shell only, no sampling |
| 2026-10-24 | 0.1.0 (M1a) | 1000 ms | 0.418 | 0.706 | 6.3 | 1.2 | CPU + memory only |
| 2026-10-24 | 0.1.0 (M1b) | 1000 ms | 1.426 | 2.008 | 12.4 | 5.4 | + disk (PDH) and network; gate met (mean < 1.5 ms) |

## Provider profile (release, tight loop)

```
GetSystemTimes                3 us
per-core times                3-30 us
current_mhz                   1 us
memory_status                 0.5 us
performance_counters         15 us
interfaces (GetIfTable2)   1200-1400 us   <- refreshed every 60 s only
counters (GetIfEntry2 x7)    270-310 us
disk sample (PDH)             40-55 us
volumes                       72-80 us     <- refreshed every 30 s only
full Sampler::sample         ~470 us
```

## Commands

```powershell
cargo run --release -p statline-app -- --bench 60          # gate
cargo run --release -p statline-app -- --profile 200      # per-provider cost
cargo run -p statline-app -- --probe                      # live values
```

## Notes

- `--bench` mean includes one wake-up per second; on a loaded desktop the wall-clock
  variance is ~3x the actual work. The tight-loop profile is the optimisation signal.
- Idle CPU is not meaningful until the sampler thread and strip exist; measure at M3.
