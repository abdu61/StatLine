//! Raw Win32 readings, exposed as plain data.
//!
//! Nothing in this module interprets the numbers: percentages, deltas and
//! history live in `statline-metrics`, where they can be tested without a
//! Windowstation. Keeping the split strict is what makes the sampler testable.

pub mod battery;
pub mod cpu;
pub mod disk;
pub mod dxgi;
pub mod gpu;
pub mod gpu_vendor;
pub mod memory;
pub mod net;
pub mod os;
pub mod pdh;
pub mod process;
pub mod thermal;

pub use cpu::{
    CpuTimes, average_mhz, base_mhz, current_mhz, logical_core_count, per_core_mhz, per_core_times,
    processor_name, system_times,
};
pub use gpu_vendor::{VendorReading, VendorSensors};
pub use memory::{
    MemoryStatus, PerformanceCounters, ProcessMemory, current_process_memory, memory_status,
    page_file_usage, performance_counters, process_cpu_time, uptime_secs,
};
pub use os::PlatformInfo;

use windows::Win32::Foundation::FILETIME;

/// Convert a `FILETIME` to a 64-bit count of 100 ns ticks.
pub(crate) fn filetime_to_u64(value: FILETIME) -> u64 {
    ((value.dwHighDateTime as u64) << 32) | value.dwLowDateTime as u64
}
