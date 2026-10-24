//! CPU counters: total times, per-core times and current clock.

use super::filetime_to_u64;
use std::mem::size_of;
use windows::Wdk::System::SystemInformation::{
    NtQuerySystemInformation, SystemProcessorPerformanceInformation,
};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Power::{
    CallNtPowerInformation, PROCESSOR_POWER_INFORMATION, ProcessorInformation,
};
use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
use windows::Win32::System::Threading::GetSystemTimes;
use windows::Win32::System::WindowsProgramming::SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION;

/// Processor time counters, in 100 ns ticks.
///
/// `kernel` **includes** `idle` — that is how Windows reports it, and the
/// percentage math in `statline-metrics` depends on knowing that.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CpuTimes {
    pub idle: u64,
    pub kernel: u64,
    pub user: u64,
}

/// Number of logical processors.
pub fn logical_core_count() -> u32 {
    let mut info = SYSTEM_INFO::default();
    // SAFETY: `info` is a valid, fully owned SYSTEM_INFO.
    unsafe { GetSystemInfo(&mut info) };
    info.dwNumberOfProcessors.max(1)
}

/// System-wide processor times.
pub fn system_times() -> Option<CpuTimes> {
    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();

    // SAFETY: all three pointers refer to live, correctly typed locals.
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;

    Some(CpuTimes {
        idle: filetime_to_u64(idle),
        kernel: filetime_to_u64(kernel),
        user: filetime_to_u64(user),
    })
}

/// Per-logical-processor times. Returns an empty vector if the query fails.
pub fn per_core_times() -> Vec<CpuTimes> {
    let cores = logical_core_count() as usize;
    let mut buffer = vec![SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION::default(); cores];
    let bytes = (cores * size_of::<SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION>()) as u32;
    let mut returned = 0u32;

    // SAFETY: the buffer is exactly `bytes` long and correctly aligned; the
    // kernel writes at most `bytes` and reports the amount in `returned`.
    let status = unsafe {
        NtQuerySystemInformation(
            SystemProcessorPerformanceInformation,
            buffer.as_mut_ptr().cast(),
            bytes,
            &mut returned,
        )
    };
    if status.0 < 0 {
        return Vec::new();
    }

    let written = (returned as usize / size_of::<SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION>())
        .min(cores)
        .max(1);
    buffer.truncate(written);

    buffer
        .into_iter()
        .map(|info| CpuTimes {
            idle: info.IdleTime as u64,
            kernel: info.KernelTime as u64,
            user: info.UserTime as u64,
        })
        .collect()
}

/// Average current core clock in MHz, as reported by the power subsystem.
pub fn current_mhz() -> Option<u32> {
    let cores = logical_core_count() as usize;
    let mut buffer = vec![PROCESSOR_POWER_INFORMATION::default(); cores];
    let bytes = (cores * size_of::<PROCESSOR_POWER_INFORMATION>()) as u32;

    // SAFETY: no input buffer; the output buffer is `bytes` long and aligned
    // for `PROCESSOR_POWER_INFORMATION`.
    let status = unsafe {
        CallNtPowerInformation(
            ProcessorInformation,
            None,
            0,
            Some(buffer.as_mut_ptr().cast()),
            bytes,
        )
    };
    if status.0 < 0 {
        return None;
    }

    let total: u64 = buffer.iter().map(|info| info.CurrentMhz as u64).sum();
    if total == 0 {
        None
    } else {
        Some((total / cores as u64) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_at_least_one_core() {
        assert!(logical_core_count() >= 1);
    }

    #[test]
    fn system_times_are_monotonic_and_nonzero() {
        let first = system_times().expect("GetSystemTimes");
        std::thread::sleep(std::time::Duration::from_millis(50));
        let second = system_times().expect("GetSystemTimes");
        assert!(second.idle >= first.idle);
        assert!(second.kernel >= first.kernel);
        assert!(second.user >= first.user);
        assert!(second.kernel > 0);
    }

    #[test]
    fn per_core_count_matches_logical_processors() {
        let cores = per_core_times();
        assert!(!cores.is_empty());
        assert!(cores.len() <= logical_core_count() as usize);
    }

    #[test]
    fn mhz_is_plausible_when_reported() {
        if let Some(mhz) = current_mhz() {
            assert!(mhz > 100, "implausible clock: {mhz} MHz");
            assert!(mhz < 10_000, "implausible clock: {mhz} MHz");
        }
    }

    #[test]
    fn filetime_conversion_combines_halves() {
        let value = FILETIME {
            dwLowDateTime: 0x0000_0001,
            dwHighDateTime: 0x0000_0002,
        };
        assert_eq!(filetime_to_u64(value), 0x0000_0002_0000_0001);
    }
}
