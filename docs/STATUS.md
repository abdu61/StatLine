# Status

**Milestone:** M1 — metrics engine · **Updated:** 2026-10-24

## Now

- [x] `Snapshot` model, units/formatting, ring buffer (49 tests total, all green)
- [x] CPU: total (`GetSystemTimes`), per-core (`NtQuerySystemInformation`), MHz
- [x] Memory: `GlobalMemoryStatusEx` + `GetPerformanceInfo`
- [x] `--probe` and `--bench`, console attach for release builds
- [ ] Network (`GetIfTable2`)
- [ ] Disk (cached PDH query, English counters)
- [ ] GPU (PDH `GPU Engine` + DXGI VRAM)
- [ ] Battery / thermal
- [ ] Sampler thread + waitable timer + `ArcSwap` publish

## Next (in order)

1. Network + disk providers, wired into `Snapshot`, shown by `--probe`.
2. GPU + battery, each degrading to `None` when unavailable.
3. Sampler thread with waitable timer; `--bench` measures through the thread.
4. M2: Direct2D software surface + headless snapshot harness.

## Blockers / decisions needed

- None.

## Measured facts (this machine)

Ryzen 7 4700U, 8C/8T, 7.4 GiB RAM · Windows 11 25H2 build 26220.9568 · 1536×864 logical
@125 % · taskbar bottom, left-aligned, 48 logical px · `MSTaskSwWClass` absent (XAML) ·
`PrintWindow` captures the taskbar, `CopyFromScreen` does not.

Bench (release, 20 samples @ 1 s): mean 0.418 ms, p95 0.706 ms, working set 6.3 MB,
private 1.2 MB. Budget: p95 < 1.5 ms, WS < 30 MB → within budget.

Note: `CallNtPowerInformation(ProcessorInformation)` reports a static 2.0 GHz on this
AMD part, same as WMI. If it stays static, M1 will switch the label to "base clock" or
derive the effective clock from PDH `% Processor Performance`.

## Log

- 2026-10-24 — M0 committed: workspace, tray shell, docs index, capture tooling.
- 2026-10-24 — M1 slice 1: core model/units/ring, CPU+memory providers, probe/bench.
