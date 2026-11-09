# ADR 0004 — Settings live in `%APPDATA%`, logs in `%LOCALAPPDATA%`

**Status:** accepted · 2026-10-24

## Decision

| What | Where | Why |
|---|---|---|
| `settings.json` | `%APPDATA%\StatLine\settings.json` | Roams with the user profile; user-editable for power users and bug reports. |
| Logs | `%LOCALAPPDATA%\StatLine\logs\statline.log` | Machine-local, disposable, never roamed. |
| Snapshots (dev only) | caller-supplied path | Produced by `--render-snapshots`, never written by the app on its own. |

Nothing else is written. No registry keys (except the documented autostart value under
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`), no ProgramData, no temp files
left behind.

## File rules

- **Versioned:** `schema_version` at the root; migrations are a function per version.
- **Atomic:** write `settings.json.tmp`, flush, then `MoveFileEx(..., MOVEFILE_REPLACE_EXISTING)`.
- **Recoverable:** a file that fails to parse is renamed to `settings.json.corrupt`
  (never deleted) and defaults are used.
- **Forward-compatible:** unknown keys are preserved on rewrite so a newer build's
  settings are not destroyed by a downgrade.
- **Human-readable:** pretty-printed JSON, stable key order, no binary blobs.

## Consequences

MSIX-packaged builds get virtualized `%APPDATA%`/`%LOCALAPPDATA%` paths automatically;
the same code works in both portable and packaged form because we ask the environment
rather than hardcoding a path.
