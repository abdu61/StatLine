# Performance log

Append one line per measured run. Machine: see `environment.md`.

| Date | Build | Interval | Idle CPU % | WS MB | Private MB | Sample p95 | Notes |
|---|---|---|---|---|---|---|---|
| 2026-10-24 | 0.1.0 (M0) | — | not measured | 10.0 | 1.5 | — | tray-only shell, no sampling yet; 4 threads, 137 handles, exe 0.16 MB |
| 2026-10-24 | 0.1.0 (M1a) | 1000 ms | not measured | 6.3 | 1.2 | 0.706 ms | CPU + memory only, no UI; `--bench 20` |

## Commands

```powershell
# sampling cost + memory (release)
cargo run --release -p statline-app -- --bench 20

# live values
cargo run -p statline-app -- --probe

# shell footprint of the tray build
$p = Start-Process ".\target\release\statline.exe" -PassThru
Start-Sleep -Milliseconds 1500
$proc = Get-Process -Id $p.Id
"{0:N1} {1:N1}" -f ($proc.WorkingSet64/1MB), ($proc.PrivateMemorySize64/1MB)
Stop-Process -Id $p.Id -Force
```

## Notes

- Working set is lower in `--bench` than in the tray build because no window, tray icon
  or message loop exists yet.
- Idle CPU is not meaningful until the sampler thread and strip exist; measure at M3.
