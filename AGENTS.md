# AGENTS.md — how to work in this repository

Read this file first. Then read `docs/STATUS.md`. Then only the spec for the area you
are touching. Do not bulk-read the docs tree; it is deliberately indexed.

## What this is

**Statline** — a native Windows 11 system monitor that lives in the taskbar. Rust,
Win32, Direct2D. No Electron, no WebView, no .NET runtime, no driver, no network
access, no telemetry.

## Non-negotiables

1. **Resource budget is a feature.** Idle CPU < 0.2 % of one core, working set < 30 MB,
   zero GPU usage at idle. Re-read `docs/ops/perf-budget.md` before adding work to the
   sampling or render path. If a change threatens the budget, it needs a note in the
   ADR and a measurement.
2. **`unsafe` lives in two crates only:** `statline-win` and the Direct2D calls in
   `statline-render`. Every block gets a `// SAFETY:` comment naming the invariant that
   makes it sound. No `unsafe` anywhere else.
3. **No panics in production paths.** `unwrap`/`expect` are for tests and genuinely
   unreachable invariants (with a comment). Use `Result` + `thiserror` in libraries,
   and log-and-continue where a single metric provider fails.
4. **Dependencies need justification.** Append to `docs/adr/0005-dependency-policy.md`
   (license, size, why) and keep `deny.toml` green. Allowed licenses: MIT, Apache-2.0,
   BSD-2/3, ISC, Zlib. No GPL/AGPL/SSPL/LGPL in the shipped binary.
5. **Never copy code from other monitor projects.** TrafficMonitor ("Anti 996" License),
   Deskband11 and TaskbarWidgets (MIT) are behavioural references only. Techniques and
   Win32 API usage are fine; their source is not.
6. **No network in v1.** Not even an update check. If that changes, it is an ADR and a
   privacy statement change.
7. **Every user-visible string** goes through `statline-core::strings` (when it exists)
   so localization is a later drop-in, not a rewrite.

## Build & verify (run all of these before a commit)

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release
```

Visual changes additionally need a snapshot:

```powershell
cargo run -p statline-app -- --render-snapshots .\snapshots-out
# and for live taskbar layout:
powershell -ExecutionPolicy Bypass -File .\scripts\capture-window.ps1
```

Read the produced PNGs. Do not claim a UI change works without looking at pixels.

## Commits

Small, conventional, local only. **Never push.**

```
feat(metrics): add per-core CPU sampling
fix(taskbar): keep strip aligned after DPI change
docs(status): mark M3 complete
```

Update `docs/STATUS.md` and the milestone file in `docs/status/` in the same commit as
the work they describe.

## Working protocol

1. `docs/STATUS.md` → current milestone + next unchecked item.
2. Read the relevant spec in `docs/spec/` (one file, not all).
3. Implement, test, measure.
4. Update status docs, commit.
5. If blocked by something only a human can do (elevation, network, name availability),
   write `docs/BLOCKED.md` with the precise question, commit everything that works, and
   move to the next independent task.

## Layout

| Path | Responsibility |
|---|---|
| `crates/statline-core` | Snapshot model, module ids, units, formatting. Pure logic, no I/O. |
| `crates/statline-config` | Settings schema, atomic persistence, migration. |
| `crates/statline-win` | All Win32 glue: windows, tray, taskbar, DPI, theme, power. |
| `crates/statline-metrics` | Providers + sampler thread. Benchmarked in isolation. |
| `crates/statline-render` | Direct2D/DirectWrite/WIC, tokens, widget kit, charts. |
| `crates/statline-ui` | Views + pure layout/hit-test math. |
| `crates/statline-app` | Composition root, message loop, CLI. |
| `docs/` | See `docs/INDEX.md`. |
