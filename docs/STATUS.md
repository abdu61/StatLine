# Status

**Milestone:** M1 complete — metrics engine · **Updated:** 2026-10-24

## Now

- [x] `Snapshot` model, units/formatting, ring buffer
- [x] CPU total/per-core/MHz, memory/commit/cache, disk, network
- [x] GPU utilization + VRAM (PDH `GPU Engine` + `GPU Process Memory`, DXGI budget)
- [x] Battery percent/AC/rate; desktops correctly report `None`
- [x] Sampler thread: condvar-paced, `ArcSwap` publish, live interval change, clean stop
- [x] `--probe`, `--bench` (CPU-time gate), `--profile` (per-provider cost)
- [x] 68 tests green, clippy clean
- [ ] M2: Direct2D software surface, design tokens, widget kit, headless snapshots

## Next (in order)

1. M2: `statline-render` — D2D/DWrite/WIC factories, DIB surface, palette + type scale.
2. M2: widget kit (text, meter, sparkline, card, button, toggle) + `--render-snapshots`.
3. M3: taskbar strip window lifecycle, placement, fallbacks, live capture QA.
4. M4: tray menu, single instance, autostart, crash handling.

## Blockers / decisions needed

- None.

## Measured facts (this machine)

Ryzen 7 4700U (8C/8T), 7.4 GiB RAM, Radeon iGPU · Windows 11 25H2 build 26220.9568 ·
1536x864 logical @125 % · taskbar bottom, left-aligned, 48 logical px · `MSTaskSwWClass`
absent (XAML taskbar) · `PrintWindow` captures the taskbar, `CopyFromScreen` does not.

Sampling cost (release, 60 samples @ 1 s): **cpu mean 0.78 ms/sample = 0.078 % of one
core**, WS 14.3 MB, private 6.2 MB. Tight-loop full sample ~0.56 ms.

Provider costs (release, tight loop): `GetIfTable2` 1.16-1.4 ms (cached 60 s),
`GetIfEntry2` x7 ~0.27-0.33 ms, GPU PDH+DXGI ~0.65 ms (cached 3 s), PDH disk ~50 us,
battery ~12 us, volumes ~76 us (cached 30 s).

## Log

- 2026-10-24 — M0 committed: workspace, tray shell, docs index, capture tooling.
- 2026-10-24 — M1 slice 1: core model/units/ring, CPU+memory providers, probe/bench.
- 2026-10-24 — M1 slice 2: disk (PDH) + network providers, interface/volume caching.
- 2026-10-24 — M1 slice 3: GPU (PDH + DXGI), battery, sampler thread; CPU-time bench gate.
