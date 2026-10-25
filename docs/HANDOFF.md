# Statline — handoff

Native Windows 11 system monitor that lives in the taskbar. Rust + Win32 + Direct2D.
Repo: `c:\Users\abdu1\Desktop\Windows Perf Monitor`. MIT. Local commits only, never push.

Read `AGENTS.md`, then `docs/STATUS.md`, then this file. Do not bulk-read the docs tree.

## Build & verify (run before every commit)

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Kill a running instance first (`Get-Process statline | Stop-Process -Force`) or the
linker cannot replace the exe.

## Run modes

```powershell
.\target\release\statline.exe                    # strip + tray (default)
.\target\release\statline.exe --tray-only        # no strip
.\target\release\statline.exe --probe            # live values, 5 samples
.\target\release\statline.exe --bench 60         # sampling cost + memory
.\target\release\statline.exe --profile 200      # per-provider tight-loop cost
.\target\release\statline.exe --render-snapshots .\snapshots-out
.\target\release\statline.exe --install-startup | --remove-startup | --startup-status
```

Log: `%LOCALAPPDATA%\Statline\logs\statline.log` (1 MB, 3 rotations).
Single instance is enforced by a named mutex; a second launch logs and exits.

## Visual QA — important

**Screen capture cannot see this project on this machine.** `CopyFromScreen` and
`BitBlt` omit both the taskbar and layered windows here. Do not conclude "invisible"
from a screenshot.

- Review the design with `--render-snapshots <dir>` and read the PNGs (they render the
  real drawing code, composited over a taskbar colour).
- Verify live windows logically. The strip is now a **child** of the taskbar, so
  `FindWindow` (which skips children) will not find it:

  ```powershell
  $taskbar = [User32]::FindWindow("Shell_TrayWnd", $null)
  $strip   = [User32]::FindWindowEx($taskbar, [IntPtr]::Zero, "StatlineStripWindow", "Statline")
  ```

  Pass the real title: an empty string matches only windows whose title is empty, which
  is a confusing way to get a zero handle.

  `FindWindow("StatlineFlyoutWindow", "Statline")` and
  `FindWindow("StatlineCompanionWindow", "Statline")` still work: both are top-level.
- Check `GetParent(strip)`, `IsWindowVisible`, `GetWindowRect`, the `WS_CHILD` and
  `WS_EX_LAYERED` style bits, and the log line `presented=true tier=child`.
- Post synthetic input to test interaction: `PostMessage(strip, 0x0202 /*LBUTTONUP*/, 0,
  MAKELPARAM(x, y))`. Clicking the strip must create the flyout; a second click closes
  it. Reading the log alone is enough: `flyout window created` / `flyout closed`.
- Capturing the companion (the only non-layered surface): `PrintWindow(hwnd, hdc, 1)`
  works, but a DPI-unaware script gets a virtualized, cropped bitmap. Call
  `SetProcessDPIAware()` before `GetClientRect`, and the capture matches the layout.
  The strip and flyout are layered; capture those with `--render-snapshots` only.

## Layout

```
crates/statline-core     model, units/formatting, ring buffer        (pure, no I/O)
crates/statline-config   settings schema + atomic persistence        (not yet wired)
crates/statline-win      ALL unsafe Win32: window, tray, menus, taskbar, strip window,
                         single instance, autostart, logger, metrics wrappers
crates/statline-metrics  providers + sampler thread (ArcSwap publish)
crates/statline-render   D2D/DWrite surface, colour, PNG writer, tokens
crates/statline-ui       strip model+render, flyout model+render, dashboard render
crates/statline-app      bin: main, cli, profile, strip_host, flyout_host, dashboard_host
docs/                    INDEX.md, STATUS.md, adr/, spec/, ops/, status/
```

## State (all committed, 133 tests green, clippy clean)

- **M0–M2 done**: workspace, docs, tray shell; metrics (CPU total/per-core/MHz, memory,
  PDH disk, per-adapter network, GPU util+VRAM, battery); software Direct2D renderer
  with headless PNG snapshots.
- **M3 done**: strip renders in the taskbar beside the tray at 237x42 (125 %), segments
  CPU/MEM/GPU/DISK + two-line NET, per-segment hover, right-click menu, hides for
  fullscreen and a hidden taskbar, recreates on `TaskbarCreated`. **Parented into
  `Shell_TrayWnd`** (`WS_CHILD`, tier 1) with a verified parent link; if the parent link
  is refused or the first present on the child fails, the host detaches to the topmost
  tier automatically.
- **M4 done**: single instance, autostart commands, log rotation, panic hook, tray menu
  with Pause/Resume (parks the sampler on its condvar), Show/Hide strip, Dashboard, Exit.
- **M5 partial**: flyout opens on strip click, toggles on a second click, closes on Esc
  or focus loss. A segment focuses one module (CPU per-core bars and CPU process table,
  Memory commit/cache and top memory apps, Disk volumes, Network adapters) plus System
  and Battery; the background click shows the overview. Charts use the accent and turn
  yellow/red at the module thresholds. It is created at its final anchored rect and
  re-anchors upward when the process table arrives; Mica makes the panel translucent.
- **M6 partial**: companion window is a real `WS_OVERLAPPEDWINDOW` (caption, system menu,
  resizable, Windows 11 dark frame + rounded corners). Left nav with Dashboard, CPU,
  Memory, GPU, Disk, Network, Battery, System, Settings and About; each module has a page
  built from the focused flyout model. CPU adds 1/5/15-minute averages; the battery card
  shows health, temperature and cycle count. Optional DWM Mica backdrop on framed windows.
- **Per-app breakdown**: `NtQuerySystemInformation(SystemProcessInformation)` with CPU-time
  deltas and working sets. Task Manager scale (share of all cores); `None` until a baseline
  exists, rendered `—`. Sampled at 2 s only while the flyout or companion is open, and
  zero when both are closed.
- **Settings**: framed window from the tray/strip menu or the companion nav. Theme
  (system/light/dark), accent presets (system/blue/green/purple/orange/teal/pink),
  interval, byte units, Mica, strip placement, autostart and per-module strip visibility,
  applied live and saved atomically to `%APPDATA%\Statline\settings.json`. Turning Mica
  on shows a GPU/battery caution. The Run key is treated as the truth for startup.
- **Sensors**: ACPI zones via PDH (5 s, whole Kelvin), drive temperature via
  `IOCTL_STORAGE_QUERY_PROPERTY` with a volume→disk mapping (5 min cache, ~11 ms),
  battery health/temperature/cycles via `IOCTL_BATTERY_QUERY_INFORMATION` (30 s), and
  GPU vendor telemetry via lazily loaded NVML/ADL (3 s GPU cadence; AMD PMLog verified
  on this APU, Overdrive5 fallback; NVML needs an NVIDIA machine to exercise). They
  share the process table's visibility gate and hide when unavailable.
- **Packaging**: `scripts/package.ps1` builds `dist/Statline-<v>-x64.zip` (exe, README,
  changelog, licence texts) plus a SHA-256. `scripts/build-msix.ps1` stages the layout and
  runs the SDK's makeappx; the manifest declares `runFullTrust` and a startup task, and
  the tile assets live under `packaging/msix/Assets`. CI has a tag-only packaging job.

Measured (release): 0.46 ms CPU per sample in a tight loop with detail providers off;
tray-only 10.6 MB WS / 1.8 MB private; parented strip 30.0 MB / 14.5 MB / 314 handles,
flat over 90 s. Process table: **~4.75 ms per query** (~250 processes). With a detail
surface open the average is ~2.3 ms/sample; sensors add 1.16 ms per 5 s (ACPI), 0.14 ms
per 30 s (battery) and ~11 ms per 5 min (drive temps).

## Next work, in priority order

1. **Settings, remaining**: module reorder, density, safe mode, remembered window
   bounds.
2. **GPU sensors**: implemented for NVML/ADL. An NVIDIA machine is still needed to
   exercise the NVML path; AMD is verified on this APU. CPU package temperature and fan
   RPM need a kernel driver and stay out of scope.
3. **Packaging, remaining**: a multi-size `.ico` for the executable/tray, the one-hour
   soak and `--bench 300`, then the local annotated `v0.1.0` tag. Store/trademark checks
   are human tasks.
4. **Polish**: tooltips, keyboard path, empty states, `--safe-mode`, multi-monitor,
   `WM_DPICHANGED`, Mica/acrylic on the companion (needs a non-layered window + DWM
   backdrop; today it paints an opaque background).

## Gotchas that cost hours — do not relearn these

- **`UPDATELAYEREDWINDOWINFO.pblend` must not be null** with `ULW_ALPHA`. A null pointer
  makes `UpdateLayeredWindowIndirect` fail with `ERROR_GEN_FAILURE (0x8007001F)`, which
  reads like a broken display driver. It also made tier 1 look impossible for a while —
  the parented child was blamed. Pass `BLENDFUNCTION { AC_SRC_OVER, 0, 255,
  AC_SRC_ALPHA }` before concluding anything about the window hierarchy.
- **A parented strip is a child window.** Position it in taskbar-client coordinates
  (`ScreenToClient`), handle `TaskbarCreated` in a top-level window (broadcasts skip
  children), and look it up with `FindWindowEx`, not `FindWindow`.
- **Never store a pointer to a value you then move.** The strip host is boxed exactly
  because the window procedure reads it through a static pointer.
- **GDI destruction must follow Direct2D teardown.** `GdiSurface` drops after the D2D
  fields on purpose; reversing that corrupts the heap later, far from the cause.
- **`GetLastError` must be cleared before `CreateMutexW`**, otherwise a stale error reads
  as "another instance is running". Use a plain mutex name (no `Local\` prefix).
- **Moving a layered window every tick swallows clicks** — mouse-down lands on one frame
  and mouse-up on the next. Only `MoveWindow` when the rect actually changes.
- **windows-rs 0.62 hides `UserTime` / `KernelTime`** in `SYSTEM_PROCESS_INFORMATION`
  behind `Reserved1: [u8; 48]`. Define the documented `repr(C)` prefix locally and walk
  the `NextEntryOffset` chain with `read_unaligned`. The query costs **~4.8 ms** for
  ~250 processes: never run it unless a surface showing processes is open.
- **The ACPI thermal counter is whole Kelvin.** `\Thermal Zone Information(*)\Temperature`
  returned 350 here, which is 77 °C — not 35 °C. `deci-Kelvin` assumptions produce −238 °C.
  Convert with `value − 273.15` and discard anything outside −20..125 °C.
- **Device IOCTLs are not cheap.** Mapping a volume to a disk and reading its temperature
  costs ~11 ms; cache it for minutes. Open the volume/disk with zero access and
  `FILE_SHARE_READ|WRITE`. The battery device is the exception: it needs
  `GENERIC_READ|GENERIC_WRITE`, and some firmware refuses `BatteryTemperature` with
  `ERROR_INVALID_FUNCTION` — hide the value.
- **Gate every expensive provider behind surface visibility.** Process table, ACPI zones,
  battery health and drive temps share `set_detail_surfaces`; with only the strip visible
  the tight-loop sample is 0.46 ms. Opening a detail surface costs ~2.3 ms/sample.
- **MSIX/MakeAppx details**: manifests are schema-validated, so the identity version must
  have four parts (`0.1.0.0`), the three namespaces (`uap`, `desktop`, `rescap`) must be
  declared, and `runFullTrust` is required for a full-trust desktop app. The script finds
  `makeappx.exe` under `Windows Kits\10\bin\...\x64`. Sideloading an unsigned package
  fails; sign with a trusted dev certificate.
- **Mica is frame-only on Win32.** `DWMWA_SYSTEMBACKDROP_TYPE = 2` applies to normal
  framed windows (companion/settings). The strip and flyout are layered (ULW); the
  flyout becomes lightly translucent instead, and per-pixel blur would require a
  composition swapchain. Turning it on shows a GPU/battery caution in Settings.
- **GUI-subsystem CLI output can vanish.** The release binary has no console; `--probe`
  and friends attach to the parent console, and that output races process exit when the
  command is run in a loop. An empty `--startup-status` line is not "disabled": read
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` or assert it in a Rust test.
- **`Edit`-style PowerShell string surgery corrupts source files.** An array-flattening bug
  once replaced every `s` with `t`. Use the file tools; if using PowerShell, do single
  `.Replace()` calls and write with `UTF8Encoding($false)`. Never round-trip a file with
  `Get-Content`/`Set-Content` on non-ASCII content.
- **MSIX/Store**: there is no supported third-party taskbar-widget API. Microsoft's only
  channel is a Widgets Board provider (Windows App SDK + MSIX), which renders in the
  Widgets flyout, not the taskbar. The overlay design stands.

## Conventions

- `unsafe` only in `statline-win` and the D2D calls in `statline-render`; every block gets
  a `// SAFETY:` comment naming the invariant.
- No `unwrap`/`expect`/`panic!` outside tests and documented invariants. Providers return
  `Option` and the UI hides missing rows.
- Conventional commits, small and frequent, local only. Update `docs/STATUS.md` and the
  milestone file in `docs/status/` in the same commit as the work.
- Perf budget is a merge gate: `docs/ops/perf-budget.md` (cpu mean < 1.5 ms/sample;
  working set < 35 MB with the D2D/DWrite delta documented).
