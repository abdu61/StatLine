//! Physical memory, system-wide performance counters and process accounting.

use std::mem::size_of;
use std::time::Duration;
use windows::Wdk::System::SystemInformation::{NtQuerySystemInformation, SYSTEM_INFORMATION_CLASS};
use windows::Win32::Foundation::{FILETIME, UNICODE_STRING};
use windows::Win32::System::ProcessStatus::{
    GetPerformanceInfo, GetProcessMemoryInfo, PERFORMANCE_INFORMATION, PROCESS_MEMORY_COUNTERS,
};
use windows::Win32::System::SystemInformation::{
    GetSystemInfo, GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO,
};
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

use super::filetime_to_u64;

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

/// `SYSTEM_PAGEFILE_INFORMATION` entry, laid out as the kernel writes it.
///
/// `total_size` and `total_in_use` are in **pages**; `next_entry_offset` walks
/// the variable-length list.
#[repr(C)]
struct PageFileInfo {
    next_entry_offset: u32,
    total_size: u32,
    total_in_use: u32,
    peak_usage: u32,
    page_file_name: UNICODE_STRING,
}

const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004u32 as i32;
/// `SystemPageFileInformation` (0x12), not exported as a named enum variant by
/// windows-rs 0.62.
const SYSTEM_PAGE_FILE_INFORMATION: SYSTEM_INFORMATION_CLASS = SYSTEM_INFORMATION_CLASS(0x12);

/// Aggregate page-file usage in bytes, or `None` when no page file exists.
///
/// Windows has no documented Win32 API for this; `SystemPageFileInformation`
/// returns one entry per page file. The query is a few microseconds and is
/// called with the memory sample, not from the strip-only path.
pub fn page_file_usage() -> Option<(u64, u64)> {
    let mut buffer = vec![0u64; 512];
    loop {
        let mut returned = 0u32;
        let bytes = (buffer.len() * size_of::<u64>()) as u32;
        // SAFETY: the buffer is 8-byte aligned and `bytes` bytes long; the
        // kernel writes at most that many and reports the amount.
        let status = unsafe {
            NtQuerySystemInformation(
                SYSTEM_PAGE_FILE_INFORMATION,
                buffer.as_mut_ptr().cast(),
                bytes,
                &mut returned,
            )
        };
        if status.0 == 0 {
            break;
        }
        if status.0 != STATUS_INFO_LENGTH_MISMATCH || buffer.len() >= 1 << 16 {
            return None;
        }
        buffer.resize(buffer.len() * 2, 0);
    }

    let mut system = SYSTEM_INFO::default();
    // SAFETY: `system` is a valid, fully owned SYSTEM_INFO.
    unsafe { GetSystemInfo(&mut system) };
    let page = u64::from(system.dwPageSize);
    if page == 0 {
        return None;
    }

    let length = buffer.len() * size_of::<u64>();
    let base = buffer.as_ptr().cast::<u8>();
    let mut offset = 0usize;
    let mut total = 0u64;
    let mut used = 0u64;

    loop {
        if offset + size_of::<PageFileInfo>() > length {
            break;
        }
        // SAFETY: `offset` is within the aligned buffer and the struct matches
        // the kernel layout.
        let info = unsafe { &*(base.add(offset).cast::<PageFileInfo>()) };
        total = total.saturating_add(u64::from(info.total_size).saturating_mul(page));
        used = used.saturating_add(u64::from(info.total_in_use).saturating_mul(page));
        if info.next_entry_offset == 0 {
            break;
        }
        offset += info.next_entry_offset as usize;
    }

    (total > 0).then_some((used.min(total), total))
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

/// Kernel + user CPU time consumed by this process so far.
///
/// This is the honest cost metric: it excludes scheduling delays and page-ins
/// caused by other work on the machine.
pub fn process_cpu_time() -> Option<Duration> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();

    // SAFETY: all four pointers refer to live locals; the pseudo-handle is
    // always valid for the current process.
    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
    }
    .ok()?;

    // 100 ns ticks, matching the FILETIME unit.
    let ticks = filetime_to_u64(kernel) + filetime_to_u64(user);
    Some(Duration::from_nanos(ticks * 100))
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
    fn page_file_usage_is_consistent() {
        if let Some((used, total)) = page_file_usage() {
            assert!(total > 0);
            assert!(used <= total);
        }
    }

    #[test]
    fn process_memory_is_measurable() {
        let memory = current_process_memory().expect("GetProcessMemoryInfo");
        assert!(memory.working_set_bytes > 0);
        assert!(memory.private_bytes > 0);
    }

    #[test]
    fn process_cpu_time_only_grows() {
        let first = process_cpu_time().expect("GetProcessTimes");
        let mut total = 0u64;
        for value in 0..1_000_000u64 {
            total = total.wrapping_add(value);
        }
        std::hint::black_box(total);
        let second = process_cpu_time().expect("GetProcessTimes");
        assert!(second >= first);
    }

    #[test]
    fn uptime_is_nonzero() {
        assert!(uptime_secs() > 10);
    }
}
