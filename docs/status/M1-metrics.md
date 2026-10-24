# M1 — metrics engine

**Status:** complete · 2026-10-24

## Checklist

- [x] `Snapshot` model in `statline-core` (`Option` = unavailable, never zero)
- [x] Units + formatting (binary/decimal bytes, rates, %, duration, clock) + tests
- [x] Ring buffer for chart series + tests
- [x] CPU total (`GetSystemTimes`), per-core (`NtQuerySystemInformation`), MHz
- [x] Memory (`GlobalMemoryStatusEx` + `GetPerformanceInfo`)
- [x] Disk: one cached PDH query (English counter paths) + volume capacity
- [x] Network: cached interface list + `GetIfEntry2` per tick
- [x] GPU: PDH wildcard engines + GPU process memory, DXGI budget
- [x] Battery: `GetSystemPowerStatus` + `CallNtPowerInformation`
- [x] Sampler thread: condvar-paced, `ArcSwap<Snapshot>`, live interval change
- [x] `--probe`, `--bench`, `--profile`

## Decisions worth remembering

- **Kernel time includes idle time.** `busy = (kernel + user) - idle`, `total = kernel + user`.
  A test pins this with hand-computed values; I got it wrong the first time and the test
  caught it.
- **`GetIfTable2` costs ~1.2 ms**, so interfaces are enumerated every 60 s and read with
  `GetIfEntry2` per tick. Sampling with the full table blew the budget by 4x.
- **GPU wildcard counters cost ~0.65 ms** per collection; cached for 3 s.
- **Volume capacity** cached for 30 s.
- **Bench gates on process CPU time**, not wall clock: one 1 s operation is dominated by
  scheduler jitter on a loaded desktop. Wall mean is still printed for reference.
- PDH counters are added with `PdhAddEnglishCounterW` so a localized Windows does not
  break the paths.
- No provider panics. A failed provider yields `None` and the row is hidden.

## Evidence

```
cargo test           -> 68 tests green
cargo clippy -D warnings -> clean
--probe              -> cpu/mem/disk/gpu/net/battery/procs all populated
--bench 60 @ 1 s     -> wall mean 1.88 ms, p95 3.45 ms
                        cpu mean 0.781 ms  (0.078 % of one core)
                        working set 14.3 MB, private 6.2 MB
--profile 60         -> interfaces 1161 us, GetIfEntry2 x7 333 us,
                        gpu 650 us, disk 49 us, battery 12 us, volumes 80 us
```

## Open issues

- CPU MHz is static (2.0 GHz) on this Ryzen 4700U, same as WMI. Options: label it "base"
  or derive the effective clock from PDH `% Processor Performance`. Decide when the PDH
  frequency counter is added (not required for M2/M3).
- GPU utilization sums all engine instances; on multi-GPU systems this mixes adapters.
  Acceptable for v1; revisit when per-adapter selection exists.
- ACPI thermal zone (labelled "System") not implemented yet. Deliberately deferred to M7
  with the other driver-free sensors.
