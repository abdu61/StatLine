# Architecture

## Shape

One process, two threads, no async runtime.

```
┌──────────────────────────── UI thread ────────────────────────────┐
│  Win32 message loop                                               │
│    ├─ tray callbacks / context menu                               │
│    ├─ taskbar strip window  ── hover, click, right-click          │
│    ├─ flyout window         ── anchored above the strip           │
│    └─ dashboard window      ── Overview / Modules / Appearance    │
│                                                                   │
│  render: software Direct2D → premultiplied BGRA → UpdateLayeredWindow
└───────────────▲───────────────────────────────────────────────────┘
                │ ArcSwap<Arc<Snapshot>>   (lock-free read)
┌───────────────┴──────────── sampler thread ───────────────────────┐
│  waitable timer (default 1000 ms)                                 │
│    ├─ always:  cpu, memory, network, disk I/O, gpu, battery       │
│    ├─ on demand: process list (5 s), temp probes (5 s)            │
│    └─ ring buffers for 60–300 samples per charted series          │
└───────────────────────────────────────────────────────────────────┘
```

Rules that keep the shape honest:

- The UI thread **never** performs I/O or counter queries. It reads the last snapshot.
- The sampler **never** touches a window handle. It publishes data and nothing else.
- Providers are independent: one failing provider clears its own row and logs once,
  it does not stop the others or the sampler.
- Only the UI thread owns HWNDs. Cross-thread wakeups go through `PostMessage`.

## Crates

| Crate | Owns | May depend on |
|---|---|---|
| `statline-core` | `Snapshot`, module ids, units, formatting, ring buffer | nothing |
| `statline-config` | settings schema, defaults, atomic I/O, migration | core |
| `statline-win` | every `unsafe` Win32 call: windows, tray, DPI, taskbar, theme, power, single-instance | core |
| `statline-metrics` | providers + sampler thread | core |
| `statline-render` | D2D/DWrite/WIC, design tokens, widget kit, charts, headless PNG | core |
| `statline-ui` | views + pure layout/hit-test functions | core, render |
| `statline-app` | composition root, message loop, CLI, wiring | all |

`statline-ui` layout functions take a `Rect` + settings and return geometry or hit
regions. No HWND, no D2D object. That is what makes them unit-testable.

## Data model

```rust
struct Snapshot {
    taken_at: SystemTime,
    cpu: CpuSample,        // total, per-core, mhz
    memory: MemorySample,  // used, total, commit, cached
    gpu: Option<GpuSample>,
    disks: Vec<DiskSample>,
    network: NetworkSample,
    battery: Option<BatterySample>,
    thermal: Option<ThermalSample>,
    processes: Option<ProcessTable>, // only when a view asks for it
}
```

`Option` means "provider unavailable, hide the row" — never "zero". The UI must never
render a fabricated value.

## Lifecycle

1. `main` parses CLI, sets DPI awareness, installs the panic hook, opens the log.
2. Single-instance mutex; a second launch forwards `--settings` to the first and exits.
3. Load settings → start sampler → create the strip → add the tray icon.
4. Message loop until `WM_QUIT`.
5. Shutdown: stop the sampler, remove the tray icon, remove the taskbar child window,
   flush the log.

`TaskbarCreated` (broadcast when Explorer restarts) tears down and recreates every
shell-owned surface: taskbar strip and tray icon.

## Error strategy

- Libraries return `Result<T, E>` with `thiserror`; callers decide.
- A provider failure is data, not an error: `Option<…>` + one log line.
- The UI never blocks on an error dialog. Fatal startup errors go to the log and a
  message box, then exit non-zero.
- Panics: `panic = "abort"` in release, a panic hook writes the location to the log
  before aborting. No unwinding across FFI.
