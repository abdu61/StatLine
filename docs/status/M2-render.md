# M2 — render foundation

**Status:** not started

## Goal

One drawing stack that renders every surface, including headlessly for review.

## Checklist

- [ ] D2D factory, DWrite factory, WIC factory (lazy, single-threaded)
- [ ] Software surface: DIB section + `ID2D1DCRenderTarget` + `BindDC`
- [ ] Design tokens: palette (light/dark/accent), type scale, spacing, radii
- [ ] Theme resolution: `AppsUseLightTheme`, accent colour, `WM_SETTINGCHANGE`
- [ ] Text helpers with cached `IDWriteTextLayout`, tabular figures
- [ ] Primitives: rounded rect, stroke, meter bar, sparkline, donut, icon glyphs
- [ ] Widget kit: button, toggle, slider, tabs, scroll view, tooltip, focus ring
- [ ] `--render-snapshots <dir>`: every surface × light/dark × 100/150/200 %
- [ ] Snapshot PNGs reviewed by eye; layout invariants asserted in tests

## Exit criteria

Snapshot grid looks correct at all three DPIs in both themes, with no clipped text and
consistent spacing; rendering allocates nothing per frame beyond scratch buffers.
