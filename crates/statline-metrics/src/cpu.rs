//! CPU percentage math.
//!
//! Windows reports `kernel` time that **includes** idle time, so the busy
//! fraction is `(kernel + user - idle) / (kernel + user)`. Getting this wrong
//! is the classic way to report ~2Ã— the real load, so the rule is asserted in
//! tests with hand-computed values.

use statline_win::metrics::CpuTimes;

/// Busy percentage between two readings, or `None` when the delta is unusable.
///
/// Returns `None` for the first sample, for a zero-length interval (the timer
/// fired twice within the same tick), and for counter wraparound or a
/// sleep/resume jump backwards.
pub fn busy_percent(previous: CpuTimes, current: CpuTimes) -> Option<f32> {
    let idle = current.idle.checked_sub(previous.idle)?;
    let kernel = current.kernel.checked_sub(previous.kernel)?;
    let user = current.user.checked_sub(previous.user)?;

    // `kernel` already contains `idle`; total elapsed processor time is
    // kernel + user, of which `idle` was spent idling.
    let total = kernel.saturating_add(user);
    if total == 0 {
        return None;
    }

    let busy = total.saturating_sub(idle);
    Some((busy as f64 / total as f64 * 100.0).clamp(0.0, 100.0) as f32)
}

/// Busy percentages for a set of cores, pairwise against the previous reading.
///
/// Cores that were added or removed (hot-plug, sleep/resume) are reported as
/// `None` and simply omitted from the result.
pub fn per_core_percent(previous: &[CpuTimes], current: &[CpuTimes]) -> Vec<f32> {
    previous
        .iter()
        .zip(current.iter())
        .filter_map(|(before, after)| busy_percent(*before, *after))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn times(idle: u64, kernel: u64, user: u64) -> CpuTimes {
        CpuTimes { idle, kernel, user }
    }

    #[test]
    fn half_busy() {
        // 100 ticks elapsed: 50 idle, 50 busy.
        let before = times(100, 200, 100);
        let after = times(150, 250, 150);
        assert_eq!(busy_percent(before, after), Some(50.0));
    }

    #[test]
    fn fully_busy_when_idle_does_not_advance() {
        let before = times(100, 100, 100);
        let after = times(100, 150, 150);
        assert_eq!(busy_percent(before, after), Some(100.0));
    }

    #[test]
    fn fully_idle() {
        let before = times(100, 100, 100);
        let after = times(200, 200, 100);
        assert_eq!(busy_percent(before, after), Some(0.0));
    }

    #[test]
    fn kernel_time_already_includes_idle() {
        // If kernel did NOT include idle, this delta (idle +100, kernel +100,
        // user +0) would be 50 %. Including idle makes it 0 %.
        let before = times(0, 0, 0);
        let after = times(100, 100, 0);
        assert_eq!(busy_percent(before, after), Some(0.0));
    }

    #[test]
    fn zero_interval_is_no_sample() {
        let before = times(100, 200, 100);
        assert_eq!(busy_percent(before, before), None);
    }

    #[test]
    fn backwards_counters_are_no_sample() {
        let before = times(200, 400, 200);
        let after = times(100, 200, 100);
        assert_eq!(busy_percent(before, after), None);
    }

    #[test]
    fn per_core_skips_mismatched_entries() {
        let previous = vec![times(0, 0, 0), times(0, 0, 0), times(0, 0, 0)];
        let current = vec![times(50, 100, 0), times(0, 100, 0)];
        let result = per_core_percent(&previous, &current);
        assert_eq!(result, vec![50.0, 100.0]);
    }

    #[test]
    fn percentages_stay_in_range() {
        let before = times(0, 0, 0);
        let after = times(u64::MAX, u64::MAX, u64::MAX);
        let value = busy_percent(before, after).expect("sample");
        assert!((0.0..=100.0).contains(&value));
    }
}

