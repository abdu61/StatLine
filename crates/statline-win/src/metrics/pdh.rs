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
    PDH_FMT_COUNTERVALUE, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhGetFormattedCounterValue, PdhOpenQueryA,
};
use windows::core::{PCSTR, PCWSTR};

/// `PDH_MORE_DATA`: the caller's buffer was too small, size has been updated.
const PDH_MORE_DATA: u32 = 0x8000_07D2;

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
    ///
    /// Wildcards are allowed; read them with [`PdhCounter::values`].
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

    /// Every instance value for a wildcard counter.
    ///
    /// PDH is called twice: once to learn the buffer size, once to fill it.
    /// Items without valid data (`CStatus != 0`) are skipped.
    pub fn values(&self) -> Option<Vec<f64>> {
        let mut size = 0u32;
        let mut count = 0u32;

        // SAFETY: a null buffer asks PDH for the required size; both
        // out-parameters are valid.
        let first = unsafe {
            PdhGetFormattedCounterArrayW(self.handle, PDH_FMT_DOUBLE, &mut size, &mut count, None)
        };
        if first != PDH_MORE_DATA && first != 0 {
            return None;
        }
        if size == 0 || count == 0 {
            return Some(Vec::new());
        }

        // Back the buffer with u64 words so the allocation is 8-byte aligned,
        // which is what PDH_FMT_COUNTERVALUE_ITEM_W requires.
        let words = size.div_ceil(8) as usize;
        let mut buffer = vec![0u64; words];
        let items = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();

        // SAFETY: the buffer is at least `size` bytes and suitably aligned.
        let status = unsafe {
            PdhGetFormattedCounterArrayW(
                self.handle,
                PDH_FMT_DOUBLE,
                &mut size,
                &mut count,
                Some(items),
            )
        };
        if status != 0 {
            return None;
        }

        let mut values = Vec::with_capacity(count as usize);
        for index in 0..count as usize {
            // SAFETY: PDH wrote `count` items into the buffer.
            let item = unsafe { &*items.add(index) };
            if item.FmtValue.CStatus != 0 {
                continue;
            }
            // SAFETY: PDH_FMT_DOUBLE was requested.
            values.push(unsafe { item.FmtValue.Anonymous.doubleValue });
        }
        Some(values)
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
    fn reads_wildcard_instances() {
        let query = PdhQuery::open().expect("PdhOpenQuery");
        let counter = query
            .add_english("\\GPU Engine(*)\\Utilization Percentage")
            .expect("GPU engine counter");
        assert!(query.collect());
        if let Some(values) = counter.values() {
            assert!(values.iter().all(|v| *v >= 0.0));
        }
    }

    #[test]
    fn unknown_counter_path_is_rejected() {
        let query = PdhQuery::open().expect("PdhOpenQuery");
        assert!(query.add_english("\\Not A Real Object\\Nope").is_none());
    }
}
