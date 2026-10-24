//! Physical memory and system-wide performance counters.

use std::mem::size_of;
use windows::Win32::System::ProcessStatus::{
    GetPerformanceInfo, GetProcessMemoryInfo, PERFORMANCE_INFORMATION, PROCESS_MEMORY_COUNTERS,
};
use windows::Win32::System::SystemInformation::{
    GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use windows::Win32::System::Threading::GetCurrentProcess;

/// Physical memory totals in bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryStatus {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

/// Commit, cache and object-count totals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PerformanceCounters {
    pub commit_bytes: u64,
    pub commit_limit_bytes: u64,
    pub cached_bytes: u64,
    pub processes: u32,
    pub threads: u32,
    pub handles: u32,
}

/// This process's memory usage, for `--bench`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessMemory {
    pub working_set_bytes: usize,
    pub private_bytes: usize,
    pub peak_working_set_bytes: usize,
}

/// `GlobalMemoryStatusEx`.
pub fn memory_status() -> Option<MemoryStatus> {
    let mut status = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };

    // SAFETY: `status.dwLength` is set to the struct's own size, which is the
    // contract of the API.
    unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;

    Some(MemoryStatus {
        total_bytes: status.ullTotalPhys,
        available_bytes: status.ullAvailPhys,
    })
}

/// `GetPerformanceInfo`, converted from pages to bytes.
pub fn performance_counters() -> Option<PerformanceCounters> {
    let mut info = PERFORMANCE_INFORMATION {
        cb: size_of::<PERFORMANCE_INFORMATION>() as u32,
        ..Default::default()
    };

    // SAFETY: `cb` describes the buffer we just allocated.
    unsafe { GetPerformanceInfo(&mut info, info.cb) }.ok()?;

    let page = info.PageSize as u64;
    Some(PerformanceCounters {
        commit_bytes: info.CommitTotal as u64 * page,
        commit_limit_bytes: info.CommitLimit as u64 * page,
        cached_bytes: info.SystemCache as u64 * page,
        processes: info.ProcessCount,
        threads: info.ThreadCount,
        handles: info.HandleCount,
    })
}

/// Memory usage of the calling process.
pub fn current_process_memory() -> Option<ProcessMemory> {
    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };

    // SAFETY: the pseudo-handle is always valid for the current process and
    // `cb` describes the buffer being filled.
    unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) }.ok()?;

    Some(ProcessMemory {
        working_set_bytes: counters.WorkingSetSize,
        private_bytes: counters.PagefileUsage,
        peak_working_set_bytes: counters.PeakWorkingSetSize,
    })
}

/// Seconds since the system booted.
pub fn uptime_secs() -> u64 {
    // SAFETY: no arguments, no pointers.
    let ticks = unsafe { GetTickCount64() };
    ticks / 1000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_plausible_physical_memory() {
        let status = memory_status().expect("GlobalMemoryStatusEx");
        assert!(status.total_bytes > 512 * 1024 * 1024);
        assert!(status.available_bytes <= status.total_bytes);
    }

    #[test]
    fn commit_limit_is_at_least_commit() {
        let counters = performance_counters().expect("GetPerformanceInfo");
        assert!(counters.commit_limit_bytes >= counters.commit_bytes);
        assert!(counters.processes > 0);
    }

    #[test]
    fn process_memory_is_measurable() {
        let memory = current_process_memory().expect("GetProcessMemoryInfo");
        assert!(memory.working_set_bytes > 0);
        assert!(memory.private_bytes > 0);
    }

    #[test]
    fn uptime_is_nonzero() {
        assert!(uptime_secs() > 10);
    }
}
