# ADR 0003 — Software Direct2D into layered windows

**Status:** accepted · 2026-10-24

## Context

The strip is ~200×60 px and updates once a second; the flyout is ~360×460 px and
updates while open. We need pixel-accurate text, rounded shapes, charts and per-pixel
alpha over the taskbar — with no GPU device held open and no device-loss handling.

Options:

| Option | Cost at idle | Notes |
|---|---|---|
| Direct3D + DirectComposition swapchain | GPU device alive, ~5–15 MB VRAM | Smoothest, most code, must handle `DXGI_ERROR_DEVICE_REMOVED`. |
| Direct2D HWND render target | D3D device alive | Same device-loss problem. |
| **Direct2D DC render target → DIB → `UpdateLayeredWindowIndirect`** | **zero GPU** | Software raster into premultiplied BGRA. Proven path (TrafficMonitor's `RenderType::D2D1`). |
| GDI / GDI+ | zero GPU | Poor text and alpha quality, no path gradients. |

## Decision

Render every surface with a software Direct2D DC render target bound to a
premultiplied BGRA DIB, draw text with DirectWrite, and present with
`UpdateLayeredWindowIndirect` (strip/flyout) or `StretchDIBits` in `WM_PAINT`
(dashboard, a normal window with DWM rounded corners).

Consequences of that choice:

- Redraw **only** on data change, hover change or resize. No continuous animation, so
  the CPU cost is one small raster per second.
- Same code path renders headlessly into a WIC bitmap for `--render-snapshots`, which
  is how UI changes are reviewed and regression-tested.
- No GPU usage at idle, no TDR/device-loss cases, works over RDP and in VMs.
- Accept a slightly softer look than GPU-composited text; DirectWrite ClearType at the
  target DPI keeps it crisp.

## Open stretch item

Mica on the dashboard requires a non-layered window. If we want it, the dashboard gets
a Mica-backed parent and a layered child surface. Not required for v1.
