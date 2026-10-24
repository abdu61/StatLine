# Performance log

Append one line per measured run. Machine: see `environment.md`.

| Date | Build | Interval | Idle CPU % | WS MB | Private MB | Sample p95 | Notes |
|---|---|---|---|---|---|---|---|
| 2026-10-24 | 0.1.0 (M0) | — | not measured | 10.0 | 1.5 | — | tray-only shell, no sampling yet; 4 threads, 137 handles, exe 0.16 MB |

M0 measurement command:

```powershell
$p = Start-Process ".\target\release\statline.exe" -PassThru
Start-Sleep -Milliseconds 1500
$proc = Get-Process -Id $p.Id
"{0:N1} {1:N1}" -f ($proc.WorkingSet64/1MB), ($proc.PrivateMemorySize64/1MB)
Stop-Process -Id $p.Id -Force
```
