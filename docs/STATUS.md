# Status

**Milestone:** M1 — metrics engine (CPU, memory, disk, network done) · **Updated:** 2026-10-24

## Now

- [x] `Snapshot` model, units/formatting, ring buffer
- [x] CPU total/per-core/MHz, memory, commit, cache
- [x] Disk: PDH aggregate rates + per-volume capacity (30 s cache)
- [x] Network: per-interface deltas via `GetIfEntry2` (list cached 60 s)
- [x] `--probe`, `--bench`, `--profile` (per-provider timing)
- [x] 62 tests green, clippy clean, bench mean 1.43 ms / WS 12.4 MB
- [ ] GPU (PDH `GPU Engine` + DXGI VRAM)
- [ ] Battery + thermal
- [ ] Sampler thread with waitable timer + `ArcSwap` publish

## Next (in order)

1. GPU utilization via one PDH wildcard query; VRAM via `IDXGIAdapter3`.
2. Battery via `GetSystemPowerStatus` + `CallNtPowerInformation`, ACPI thermal zone.
3. Sampler thread: waitable timer, `ArcSwap<Snapshot>`, pause/resume, interval change.
4. M2: Direct2D software surface + design tokens + headless snapshot harness.

## Blockers / decisions needed

- None.

## Measured facts (this machine)

Ryzen 7 4700U (8C/8T), 7.4 GiB RAM · Windows 11 25H2 build 26220.9568 · 1536×864
logical @125 % · taskbar bottom, left-aligned, 48 logical px · `MSTaskSwWClass` absent
(XAML taskbar) · `PrintWindow` captures the taskbar, `CopyFromScreen` does not.

Bench (release, 60 samples @ 1 s): mean 1.426 ms, p95 2.008 ms, WS 12.4 MB, private
5.4 MB. Provider profile: `GetIfTable2` 1.2-1.4 ms (cached 60 s), `GetIfEntry2` x7
~0.3 ms, PDH disk ~50 us, full sample ~0.47 ms in a tight loop.

## Log

- 2026-10-24 — M0 committed: workspace, tray shell, docs index, capture tooling.
- 2026-10-24 — M1 slice 1: core model/units/ring, CPU+memory providers, probe/bench.
- 2026-10-24 — M1 slice 2: disk (PDH) and network providers, per-provider profile,
  interface/volume caching to stay inside the sampling budget.
