# M8 — polish & robustness

**Status:** not started

## Goal

Make it feel like a finished Windows component, and fail gracefully everywhere.

## Checklist

- [ ] Hover/press/focus states audited across every control
- [ ] Tooltips with full values; keyboard path for every action
- [ ] Empty states: no GPU counters, no battery, no network adapters
- [ ] "Unsupported taskbar layout" state with a one-click switch to safe mode
- [ ] Multi-monitor behaviour reviewed (taskbar on secondary, mixed DPI)
- [ ] High-contrast sanity: text remains readable when system colours change
- [ ] Error surfaces: settings write failure, log failure, tracer arrow
- [ ] Strings extracted for future localization (`statline-core::strings`)
- [ ] Docs sweep: every spec matches the implementation

## Exit criteria

No dead ends in the UI; every failure mode has a visible, calm explanation; the
compatibility matrix in `ops/testing.md` is fully checked off.
