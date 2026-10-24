# M1 — metrics engine

**Status:** not started

## Goal

Real numbers, measured cheaply, with tests that do not need a GUI.

## Checklist

- [ ] `Snapshot` model in `statline-core` (units, ids, `Option` = unavailable)
- [ ] Units + formatting module (`12.4 MB/s`, `62 %`, `3 d 4 h`, binary/decimal) + tests
- [ ] Ring buffer for chart series + tests
- [ ] CPU total via `GetSystemTimes`, per-core via `NtQuerySystemInformation`
- [ ] CPU MHz via `CallNtPowerInformation`, registry fallback
- [ ] Memory via `GlobalMemoryStatusEx` + `GetPerformanceInfo`
- [ ] Network via `GetIfTable2` (delta, skip loopback/tunnel)
- [ ] Disk via one cached PDH query (`PdhAddEnglishCounterW`)
- [ ] GPU util via PDH wildcard + VRAM via `IDXGIAdapter3`
- [ ] Battery via `GetSystemPowerStatus` + `CallNtPowerInformation`
- [ ] Sampler thread with waitable timer, `ArcSwap` publish, gap/wrap handling
- [ ] `--probe` (one snapshot) and `--bench <n>` (cost histogram + memory)
- [ ] Provider smoke tests marked `#[ignore]` (need real hardware)

## Exit criteria

`--probe` values within ±2 % CPU / ±0.2 GB RAM of Task Manager; `--bench` p95 sample
time < 1.5 ms; all unit tests green; working set < 30 MB with no UI.
