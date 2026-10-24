# Docs index

Read in this order. Stop as soon as you have what you need.

| If you are… | Read |
|---|---|
| starting a session | [`STATUS.md`](STATUS.md) → [`../AGENTS.md`](../AGENTS.md) |
| new to the project | [`architecture.md`](architecture.md) → the relevant ADR |
| touching the taskbar window | [`adr/0002-taskbar-integration.md`](adr/0002-taskbar-integration.md) → [`spec/taskbar-integration.md`](spec/taskbar-integration.md) |
| touching drawing or layout | [`adr/0003-renderer.md`](adr/0003-renderer.md) → [`spec/ui-strip.md`](spec/ui-strip.md) / [`spec/ui-flyout.md`](spec/ui-flyout.md) / [`spec/ui-dashboard.md`](spec/ui-dashboard.md) |
| adding a metric | [`spec/metrics.md`](spec/metrics.md) |
| touching settings | [`adr/0004-config-paths.md`](adr/0004-config-paths.md) → [`spec/settings.md`](spec/settings.md) |
| adding a dependency | [`adr/0005-dependency-policy.md`](adr/0005-dependency-policy.md) |
| measuring performance | [`ops/perf-budget.md`](ops/perf-budget.md) → [`ops/testing.md`](ops/testing.md) |
| building or packaging | [`ops/build.md`](ops/build.md) → [`ops/packaging.md`](ops/packaging.md) |
| cutting a release | [`ops/release-checklist.md`](ops/release-checklist.md) |
| wondering about this machine | [`ops/environment.md`](ops/environment.md) |

## Status files

`docs/status/` holds one short file per milestone. `STATUS.md` is the only file that is
always current; milestone files are the record of what was done and why.

## Writing rules for these docs

- Facts and commands, not prose. If a sentence does not change what someone does, cut it.
- Every claim about Windows behaviour names the API or cites a link.
- Measurements include the machine, the command, and the numbers.
