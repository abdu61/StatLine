# M1 — metrics engine

**Status:** in progress (CPU + memory complete)

## Goal

Real numbers, measured cheaply, with tests that do not need a GUI.

## Checklist

- [x] `Snapshot` model in `statline-core` (`Option` = unavailable, never zero)
- [x] Units + formatting module (binary/decimal bytes, rates, %, duration, clock)
- [x] Ring buffer for chart series + tests
- [x] CPU total via `GetSystemTimes` (kernel-includes-idle rule asserted in tests)
- [x] CPU per-core via `NtQuerySystemInformation(SystemProcessorPerformanceInformation)`
- [x] CPU MHz via `CallNtPowerInformation` — static on this AMD part, see note
- [x] Memory via `GlobalMemoryStatusEx` + `GetPerformanceInfo`
- [x] `--probe` (live table) and `--bench` (p95 + working set), console attach for release
- [ ] Network via `GetIfTable2` (delta, skip loopback/tunnel)
- [ ] Disk via one cached PDH query (`PdhAddEnglishCounterW`)
- [ ] GPU util via PDH wildcard + VRAM via `IDXGIAdapter3`
- [ ] Battery via `GetSystemPowerStatus` + `CallNtPowerInformation`
- [ ] Thermal (ACPI) best effort
- [ ] Sampler thread with waitable timer, `ArcSwap` publish

## Decisions

- Raw Win32 readings live in `statline-win::metrics`; percentages and history live in
  `statline-metrics`. The split keeps `unsafe` in one crate and makes the arithmetic
  testable with synthetic counters.
- `kernel` includes `idle`; `busy = (kernel + user) − idle`, `total = kernel + user`.
  A test pins this with hand-computed values.
- First sample reports `0 %` and an empty per-core list rather than a fabricated value.

## Evidence

```
cargo test → 49 tests green (17 core, 15 win, 10 metrics, 1 app, …)

--probe
#1  cpu   0%    2.0 GHz  (warming up)  mem 5.7 GiB / 7.4 GiB (76%)  procs 246
#2  cpu  13%    2.0 GHz  8 cores, peak 19%  mem 5.7 GiB / 7.4 GiB (76%)  procs 246

--bench 20 (release)
mean 0.418 ms · p95 0.706 ms · max 0.706 ms · working set 6.3 MB · private 1.2 MB
result: within budget
```

## Open issues

- MHz is static (2.0 GHz) on the Ryzen 7 4700U. Options: label it "base", or compute
  effective clock from PDH `\Processor Information(_Total)\% Processor Performance`.
  Decide when the disk/GPU PDH query lands (same subsystem).
