//! DXGI queries: video memory in use and available.

use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL,
    DXGI_QUERY_VIDEO_MEMORY_INFO, IDXGIAdapter3, IDXGIFactory1,
};
use windows::core::Interface;

/// `(used_bytes, budget_bytes)` for the first enumerated adapter.
///
/// Integrated GPUs report their memory as non-local, so both segments are
/// summed. Returns `None` when DXGI is unavailable or the adapter does not
/// report budgets (older drivers).
pub fn video_memory() -> Option<(u64, u64)> {
    // SAFETY: standard COM creation with a checked result.
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.ok()?;

    // SAFETY: adapter index 0 is requested directly; a failure means "no GPU".
    let adapter = unsafe { factory.EnumAdapters1(0) }.ok()?;

    // SAFETY: QI for the DXGI 1.4 interface used for memory budgets.
    let adapter3: IDXGIAdapter3 = adapter.cast().ok()?;

    let mut used: u64 = 0;
    let mut budget: u64 = 0;
    for group in [
        DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
        DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL,
    ] {
        let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
        // SAFETY: `info` is a valid out-parameter; node 0 is the whole adapter.
        if unsafe { adapter3.QueryVideoMemoryInfo(0, group, &mut info) }.is_ok() {
            used = used.saturating_add(info.CurrentUsage);
            budget = budget.saturating_add(info.Budget);
        }
    }

    if budget == 0 {
        return None;
    }
    Some((used.min(budget), budget))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_memory_or_none() {
        if let Some((used, budget)) = video_memory() {
            assert!(budget > 0);
            assert!(used <= budget);
        }
    }
}
