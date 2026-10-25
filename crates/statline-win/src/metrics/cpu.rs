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
use windows::Win32::System::Registry::{
    HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
};
use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
use windows::Win32::System::Threading::GetSystemTimes;
use windows::Win32::System::WindowsProgramming::SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION;
use windows::core::{PCWSTR, w};

const CPU_KEY: PCWSTR = w!("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0");
const CPU_NAME_VALUE: PCWSTR = w!("ProcessorNameString");
const CPU_BASE_VALUE: PCWSTR = w!("~MHz");

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

/// Current clock per logical core in MHz, in processor order.
///
/// Returns an empty vector if the power subsystem refuses the query. A zero
/// entry means the core did not report a clock; callers should hide it.
pub fn per_core_mhz() -> Vec<u32> {
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
        return Vec::new();
    }

    buffer.into_iter().map(|info| info.CurrentMhz).collect()
}

/// Average of the reported per-core clocks, ignoring cores reporting zero.
pub fn average_mhz(values: &[u32]) -> Option<u32> {
    let (total, count) = values
        .iter()
        .filter(|mhz| **mhz > 0)
        .fold((0u64, 0u64), |(total, count), mhz| {
            (total + *mhz as u64, count + 1)
        });
    total.checked_div(count).map(|average| average as u32)
}

/// Average current core clock in MHz, as reported by the power subsystem.
pub fn current_mhz() -> Option<u32> {
    average_mhz(&per_core_mhz())
}

/// Processor marketing name from the registry, e.g. "AMD Ryzen 7 7840U …".
pub fn processor_name() -> Option<String> {
    let mut buffer = [0u16; 256];
    let mut size = (buffer.len() * size_of::<u16>()) as u32;

    // SAFETY: static key/value names; the buffer is `size` bytes and the output
    // size is a valid out-parameter.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            CPU_KEY,
            CPU_NAME_VALUE,
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return None;
    }

    let length = (size as usize / size_of::<u16>()).min(buffer.len());
    let name = String::from_utf16_lossy(&buffer[..length]);
    let name = name.trim_matches('\0').trim();
    (!name.is_empty()).then(|| String::from(name))
}

/// Nominal base clock in MHz from the registry.
pub fn base_mhz() -> Option<u32> {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;

    // SAFETY: static key/value names; `value` and `size` are valid DWORD
    // out-parameters.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            CPU_KEY,
            CPU_BASE_VALUE,
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    (status.is_ok() && (100..=10_000).contains(&value)).then_some(value)
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
    fn processor_identity_queries_are_plausible() {
        if let Some(name) = processor_name() {
            assert!(!name.is_empty());
            assert!(!name.contains('\0'));
        }
        if let Some(mhz) = base_mhz() {
            assert!(
                (100..=10_000).contains(&mhz),
                "implausible base clock: {mhz}"
            );
        }
    }

    #[test]
    fn average_clock_ignores_zero_entries() {
        assert_eq!(average_mhz(&[0, 3000, 1000]), Some(2000));
        assert_eq!(average_mhz(&[0, 0]), None);
        assert_eq!(average_mhz(&[]), None);
    }

    #[test]
    fn per_core_clocks_are_plausible_when_reported() {
        let values = per_core_mhz();
        assert!(values.len() <= logical_core_count() as usize);
        for mhz in values.into_iter().filter(|mhz| *mhz > 0) {
            assert!(mhz > 100 && mhz < 10_000, "implausible clock: {mhz} MHz");
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
