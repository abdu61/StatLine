# Ops — build

## Prerequisites (verified on the target machine)

| Tool | Version | Notes |
|---|---|---|
| Rust | 1.99.1 `stable-x86_64-pc-windows-msvc` | installed user-scope via `winget install Rustlang.Rustup` |
| MSVC | 14.42 (VS 2022 Community) | provides `link.exe`; no full IDE needed |
| Windows SDK | 10.0.19041 | `makeappx.exe` used for packaging |
| Git | 2.44 | local commits only, never push |

`rust-toolchain.toml` pins stable + rustfmt + clippy. If `cargo` is not on `PATH` in a
fresh shell:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

## Commands

```powershell
cargo build                     # debug
cargo build --release           # release (LTO, strip, panic=abort)
cargo test --all                # unit + integration tests
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo deny check                # licenses, advisories, duplicate versions
cargo run -p statline-app -- --render-snapshots .\snapshots-out
```

## Runtime flags (dev)

| Flag | Purpose |
|---|---|
| `--probe` | print one snapshot as a table and exit (M1) |
| `--watch` | print a live table in the console (M1) |
| `--bench <n>` | sample `n` times, report per-sample cost and memory |
| `--render-snapshots <dir>` | write PNGs of every surface/theme/DPI |
| `--tray-only` | force the tray-only tier |
| `--safe-mode` | force the topmost tier (no taskbar parenting) |
| `--settings` | focus the existing instance and open Settings |

## Troubleshooting

- `link.exe not found` → the VS Build Tools C++ workload is missing; `rustc` finds it
  through `vswhere`, but only if the MSVC toolset is installed.
- Blank strip after an Explorer restart → check the log; the `TaskbarCreated` path
  should recreate every shell surface.
- Build uses the wrong linker after switching toolchains → `rustup show` and
  `rust-toolchain.toml` must agree (`x86_64-pc-windows-msvc`).
