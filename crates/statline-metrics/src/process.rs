//! Turns the raw process table into CPU shares and the top-N list.
//!
//! The kernel reports cumulative CPU time, so a percentage needs the previous
//! reading and the wall time between the two samples. Keeping that state here
//! means the Win32 layer stays a pure reader.

use statline_core::model::{ProcessEntry, ProcessSample};
use statline_win::metrics::process::{self, RawProcess};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How often the process table is refreshed while a process view is open.
pub const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);

/// Rows kept in the snapshot. The flyout shows five, the companion up to eight.
const TOP_COUNT: usize = 12;

/// `FILETIME`-style ticks per second.
const TICKS_PER_SECOND: f64 = 10_000_000.0;

/// Cached query buffer plus the previous CPU times used for the deltas.
#[derive(Default)]
pub struct ProcessState {
    buffer: Vec<u64>,
    raw: Vec<RawProcess>,
    previous: HashMap<u32, u64>,
    last_at: Option<Instant>,
    last: Option<ProcessSample>,
}

impl ProcessState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget all delta state, so a later enable starts without a stale baseline.
    pub fn reset(&mut self) {
        self.previous.clear();
        self.last_at = None;
        self.last = None;
    }

    /// Refresh when due, otherwise return the cached sample.
    pub fn sample(&mut self, now: Instant, cores: u32) -> Option<ProcessSample> {
        let due = self
            .last_at
            .map(|last| now.duration_since(last) >= SAMPLE_INTERVAL)
            .unwrap_or(true);
        if due {
            self.take(now, cores)
        } else {
            self.last.clone()
        }
    }

    /// Sample unconditionally. Split out so tests do not have to wait 2 s.
    fn take(&mut self, now: Instant, cores: u32) -> Option<ProcessSample> {
        let elapsed = self
            .last_at
            .map(|last| now.duration_since(last))
            .unwrap_or(Duration::ZERO);
        self.last_at = Some(now);

        if !process::processes(&mut self.buffer, &mut self.raw) {
            // Keep the last good table on screen; the next tick retries.
            return self.last.clone();
        }

        let total = self.raw.len() as u32;
        let mut previous = HashMap::with_capacity(self.raw.len());
        let mut entries = Vec::with_capacity(self.raw.len().min(TOP_COUNT));

        for entry in &self.raw {
            let cpu_percent = self.previous.get(&entry.pid).map(|before| {
                cpu_percent(entry.cpu_time_100ns.saturating_sub(*before), elapsed, cores)
            });
            previous.insert(entry.pid, entry.cpu_time_100ns);
            entries.push(ProcessEntry {
                pid: entry.pid,
                name: entry.name.clone(),
                cpu_percent,
                working_set_bytes: entry.working_set_bytes,
            });
        }
        self.previous = previous;

        // Aggregate instances of the same executable, as Task Manager and Stats
        // do: Chrome's twenty renderers become one row whose CPU and memory sum.
        let mut entries = group_by_name(entries);

        // Busiest first; memory breaks ties, which also gives the first sample
        // (no CPU history yet) a useful order instead of an arbitrary one.
        entries.sort_by(|a, b| {
            b.cpu_percent
                .unwrap_or(0.0)
                .total_cmp(&a.cpu_percent.unwrap_or(0.0))
                .then_with(|| b.working_set_bytes.cmp(&a.working_set_bytes))
        });
        entries.truncate(TOP_COUNT);

        let sample = ProcessSample {
            total,
            top: entries,
        };
        self.last = Some(sample.clone());
        Some(sample)
    }
}

/// Aggregate rows that share an executable name (case-insensitive).
///
/// The representative row keeps the first PID and display name; CPU and memory
/// are summed. A group with no CPU delta stays `None` so the UI shows "—".
pub fn group_by_name(entries: Vec<ProcessEntry>) -> Vec<ProcessEntry> {
    let mut grouped: Vec<ProcessEntry> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        let key = entry.name.to_lowercase();
        if let Some(position) = index.get(&key).copied() {
            let target = &mut grouped[position];
            target.working_set_bytes = target
                .working_set_bytes
                .saturating_add(entry.working_set_bytes);
            target.cpu_percent = match (target.cpu_percent, entry.cpu_percent) {
                (Some(left), Some(right)) => Some((left + right).clamp(0.0, 100.0)),
                (Some(left), None) => Some(left),
                (None, Some(right)) => Some(right),
                (None, None) => None,
            };
        } else {
            index.insert(key, grouped.len());
            grouped.push(entry);
        }
    }

    grouped
}

/// `delta_ticks` of CPU time over `elapsed`, as a share of total capacity.
fn cpu_percent(delta_ticks: u64, elapsed: Duration, cores: u32) -> f32 {
    if delta_ticks == 0 || cores == 0 || elapsed.is_zero() {
        return 0.0;
    }
    let capacity = elapsed.as_secs_f64() * f64::from(cores);
    if capacity <= 0.0 {
        return 0.0;
    }
    let used = delta_ticks as f64 / TICKS_PER_SECOND;
    ((used / capacity) * 100.0).clamp(0.0, 100.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_share_uses_total_capacity() {
        // One saturated core for one second: 100 % on one core, 25 % on four.
        assert!((cpu_percent(10_000_000, Duration::from_secs(1), 1) - 100.0).abs() < 0.001);
        assert!((cpu_percent(10_000_000, Duration::from_secs(1), 4) - 25.0).abs() < 0.001);
        assert_eq!(cpu_percent(0, Duration::from_secs(1), 4), 0.0);
        assert_eq!(cpu_percent(10_000_000, Duration::ZERO, 4), 0.0);
        assert_eq!(cpu_percent(10_000_000, Duration::from_secs(1), 0), 0.0);
    }

    #[test]
    fn first_sample_has_no_cpu_history() {
        let mut state = ProcessState::new();
        let sample = state.take(Instant::now(), 8).expect("process table");
        assert!(sample.total > 0);
        assert!(!sample.top.is_empty());
        assert!(sample.top.len() <= TOP_COUNT);
        assert!(
            sample.top.iter().all(|entry| entry.cpu_percent.is_none()),
            "no delta exists before the second sample"
        );
    }

    #[test]
    fn second_sample_reports_bounded_sorted_cpu() {
        let mut state = ProcessState::new();
        let now = Instant::now();
        assert!(state.take(now, 8).is_some());
        std::thread::sleep(Duration::from_millis(80));
        let sample = state.take(Instant::now(), 8).expect("process table");

        assert!(!sample.top.is_empty());
        assert!(
            sample.top.iter().any(|entry| entry.cpu_percent.is_some()),
            "at least the long-lived processes must have a delta"
        );
        assert!(
            sample
                .top
                .iter()
                .all(|entry| entry.cpu_percent.is_none_or(|p| (0.0..=100.0).contains(&p)))
        );
        assert!(
            sample.top.windows(2).all(
                |pair| pair[0].cpu_percent.unwrap_or(0.0) >= pair[1].cpu_percent.unwrap_or(0.0)
            ),
            "rows must be sorted by CPU descending"
        );
    }

    #[test]
    fn grouping_sums_instances_of_the_same_name() {
        let entries = vec![
            ProcessEntry {
                pid: 1,
                name: String::from("chrome.exe"),
                cpu_percent: Some(4.0),
                working_set_bytes: 100,
            },
            ProcessEntry {
                pid: 2,
                name: String::from("CHROME.EXE"),
                cpu_percent: Some(2.5),
                working_set_bytes: 50,
            },
            ProcessEntry {
                pid: 3,
                name: String::from("explorer.exe"),
                cpu_percent: None,
                working_set_bytes: 80,
            },
        ];
        let grouped = group_by_name(entries);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].name, "chrome.exe");
        assert_eq!(grouped[0].cpu_percent, Some(6.5));
        assert_eq!(grouped[0].working_set_bytes, 150);
        assert_eq!(grouped[1].cpu_percent, None);
    }

    #[test]
    fn reset_drops_the_cached_table() {
        let mut state = ProcessState::new();
        assert!(state.take(Instant::now(), 4).is_some());
        state.reset();
        assert!(state.last.is_none());
        assert!(state.previous.is_empty());
    }
}
