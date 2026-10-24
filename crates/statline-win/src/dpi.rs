//! DPI awareness setup.

use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};

/// Opt the process into per-monitor-v2 DPI awareness.
///
/// Must run before any window is created. Failure is non-fatal: a system that
/// predates per-monitor-v2 (Windows 10 1703+) will simply use system DPI.
pub fn enable_per_monitor_v2() {
    // SAFETY: no pointers involved; the call only mutates process-wide DPI
    // state and returns a BOOL we intentionally ignore.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}
