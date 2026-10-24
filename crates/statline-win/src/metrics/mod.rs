//! Raw Win32 readings, exposed as plain data.
//!
//! Nothing in this module interprets the numbers: percentages, deltas and
//! history live in `statline-metrics`, where they can be tested without a
//! Windowstation. Keeping the split strict is what makes the sampler testable.

pub mod cpu;
pub mod disk;
pub mod memory;
pub mod net;
pub mod pdh;

pub use cpu::{CpuTimes, current_mhz, logical_core_count, per_core_times, system_times};
pub use memory::{
    MemoryStatus, PerformanceCounters, ProcessMemory, current_process_memory, memory_status,
    performance_counters, uptime_secs,
};

use windows::Win32::Foundation::FILETIME;

/// Convert a `FILETIME` to a 64-bit count of 100 ns ticks.
pub(crate) fn filetime_to_u64(value: FILETIME) -> u64 {
    ((value.dwHighDateTime as u64) << 32) | value.dwLowDateTime as u64
}
