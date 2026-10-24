//! Minimal PDH wrapper.
//!
//! Two rules matter here:
//! - Counters are added with `PdhAddEnglishCounterW`, so counter paths are
//!   never localized ("% Disk Time" stays correct on a German or Japanese
//!   Windows).
//! - One query is opened at startup and reused; the sampler never re-opens
//!   handles per tick.

use crate::to_wide;
use windows::Win32::System::Performance::{
    PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PdhAddEnglishCounterW,
    PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue, PdhOpenQueryA,
};
use windows::core::{PCSTR, PCWSTR};

/// An open PDH query. Closes itself on drop.
pub struct PdhQuery {
    handle: PDH_HQUERY,
}

impl PdhQuery {
    /// Open a query against the local machine.
    pub fn open() -> Option<Self> {
        let mut handle = PDH_HQUERY::default();
        // SAFETY: no data source (local machine) and a valid out-parameter.
        // The `A` variant is used only because the source name is null.
        let status = unsafe { PdhOpenQueryA(PCSTR::null(), 0, &mut handle) };
        if status == 0 {
            Some(Self { handle })
        } else {
            None
        }
    }

    /// Add a counter using its English path, e.g. `\PhysicalDisk(_Total)\Disk Read Bytes/sec`.
    pub fn add_english(&self, path: &str) -> Option<PdhCounter> {
        let wide = to_wide(path.as_ref());
        let mut counter = PDH_HCOUNTER::default();
        // SAFETY: the path is NUL-terminated and outlives the call.
        let status =
            unsafe { PdhAddEnglishCounterW(self.handle, PCWSTR(wide.as_ptr()), 0, &mut counter) };
        if status == 0 {
            Some(PdhCounter { handle: counter })
        } else {
            None
        }
    }

    /// Refresh every counter in the query. Rate counters need two collections
    /// before they report anything meaningful.
    pub fn collect(&self) -> bool {
        // SAFETY: `handle` was returned by PdhOpenQuery and is still open.
        unsafe { PdhCollectQueryData(self.handle) == 0 }
    }
}

impl Drop for PdhQuery {
    fn drop(&mut self) {
        // SAFETY: `handle` is owned by this struct and closed exactly once.
        unsafe {
            let _ = PdhCloseQuery(self.handle);
        }
    }
}

/// A counter that belongs to a [`PdhQuery`].
pub struct PdhCounter {
    handle: PDH_HCOUNTER,
}

impl PdhCounter {
    /// Formatted value as a double, or `None` when PDH has no valid data yet.
    pub fn value(&self) -> Option<f64> {
        let mut value = PDH_FMT_COUNTERVALUE::default();
        // SAFETY: `handle` belongs to a live query; `value` is a valid out-param.
        let status =
            unsafe { PdhGetFormattedCounterValue(self.handle, PDH_FMT_DOUBLE, None, &mut value) };
        if status != 0 || value.CStatus != 0 {
            return None;
        }
        // SAFETY: PDH_FMT_DOUBLE was requested, so the double member is active.
        Some(unsafe { value.Anonymous.doubleValue })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_a_query_and_reads_a_counter() {
        let query = PdhQuery::open().expect("PdhOpenQuery");
        let counter = query
            .add_english("\\System\\Processes")
            .expect("add English counter");
        assert!(query.collect());
        let value = counter.value().expect("process count");
        assert!(value > 0.0);
    }

    #[test]
    fn unknown_counter_path_is_rejected() {
        let query = PdhQuery::open().expect("PdhOpenQuery");
        assert!(query.add_english("\\Not A Real Object\\Nope").is_none());
    }
}
