# ADR 0005 — Dependency policy

**Status:** accepted · 2026-10-24 · **Enforced by:** `deny.toml` in CI

## Rules

1. Allowed licenses in the shipped binary: **MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause,
   ISC, Zlib, Unicode-3.0**. Anything else fails the build.
2. No GPL, AGPL, SSPL or LGPL code is linked into the binary. A future exception must be
   a separately distributed component (see the licensing audit in the plan).
3. Every new direct dependency gets a row in the table below: what it costs, why it is
   worth it, and what we would do without it.
4. Prefer the standard library, then a well-maintained single-purpose crate, then a
   framework. A framework needs a paragraph, not a line.
5. No crate that opens a network socket, a background thread at import time, or a
   hidden timer. This is a low-power app; dormant machinery is a cost.

## Current direct dependencies

| Crate | License | Size (approx) | Why |
|---|---|---|---|
| `windows` | MIT OR Apache-2.0 | build-time only, codegen per feature | First-party Win32/Direct2D/DirectWrite/WIC bindings. Generated per feature, so unused APIs cost nothing. |
| `log` | MIT OR Apache-2.0 | tiny | Backend-agnostic logging facade. |
| `thiserror` | MIT OR Apache-2.0 | tiny | Ergonomic error enums in library crates. |
| `serde` | MIT OR Apache-2.0 | small | Settings (de)serialization. |
| `serde_json` | MIT OR Apache-2.0 | small | Human-readable, diffable settings file. |

Planned, pending justification when added: `arc-swap` (lock-free snapshot publish).
Rejected so far: `tokio` (no async work to do), `sysinfo` (queries cost far more than
the targeted Win32 calls), `chrono`/`time` (a 20-line UTC formatter is enough for logs).

## Adding a dependency

1. `cargo tree -i <crate>` — check what it drags in.
2. `cargo deny check` — license + duplicate versions + advisories.
3. Add the row above, with the reason.
4. If it is a framework or a runtime, write a short ADR instead of a table row.
