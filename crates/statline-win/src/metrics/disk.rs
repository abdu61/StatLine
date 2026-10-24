//! Disk throughput (PDH) and volume capacity.

use crate::metrics::pdh::{PdhCounter, PdhQuery};
use crate::to_wide;
use std::ffi::OsStr;
use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDriveStringsW, GetVolumeInformationW,
};
use windows::core::PCWSTR;

/// `DRIVE_FIXED` from winbase.h; the constant is not generated in this binding.
const DRIVE_FIXED: u32 = 3;

/// Throughput rates for the aggregate of all physical disks.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DiskRates {
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    pub busy_percent: f32,
}

/// One open PDH query holding every disk counter we need.
pub struct DiskCounters {
    query: PdhQuery,
    read: PdhCounter,
    write: PdhCounter,
    busy: PdhCounter,
}

impl DiskCounters {
    /// Open the query and add the three aggregate counters.
    pub fn open() -> Option<Self> {
        let query = PdhQuery::open()?;
        let read = query.add_english("\\PhysicalDisk(_Total)\\Disk Read Bytes/sec")?;
        let write = query.add_english("\\PhysicalDisk(_Total)\\Disk Write Bytes/sec")?;
        let busy = query.add_english("\\PhysicalDisk(_Total)\\% Disk Time")?;
        // Prime the rate counters; the first sample has no interval.
        let _ = query.collect();
        Some(Self {
            query,
            read,
            write,
            busy,
        })
    }

    /// Refresh and read the current rates.
    pub fn sample(&self) -> Option<DiskRates> {
        if !self.query.collect() {
            return None;
        }
        Some(DiskRates {
            read_bytes_per_sec: self.read.value().unwrap_or(0.0).max(0.0),
            write_bytes_per_sec: self.write.value().unwrap_or(0.0).max(0.0),
            busy_percent: self.busy.value().unwrap_or(0.0).clamp(0.0, 100.0) as f32,
        })
    }
}

/// Capacity for one mounted fixed volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeInfo {
    /// `"C:"` — trailing separator removed for display.
    pub mount: String,
    pub label: String,
    pub free_bytes: u64,
    pub total_bytes: u64,
}

/// Enumerate fixed local volumes. Removable and network drives are skipped.
pub fn volumes() -> Vec<VolumeInfo> {
    let mut buffer = vec![0u16; 512];
    // SAFETY: the slice is a valid writable buffer; the API returns the number
    // of characters written, or the required size when too small.
    let written = unsafe { GetLogicalDriveStringsW(Some(&mut buffer)) };
    if written == 0 || written as usize > buffer.len() {
        return Vec::new();
    }

    let mut volumes = Vec::new();
    for root in split_drive_strings(&buffer[..written as usize]) {
        let wide = to_wide(OsStr::new(&root));
        let path = PCWSTR(wide.as_ptr());

        // SAFETY: `path` is NUL-terminated and outlives the call.
        if unsafe { GetDriveTypeW(path) } != DRIVE_FIXED {
            continue;
        }

        let mut free = 0u64;
        let mut total = 0u64;
        // SAFETY: all three pointers are valid out-parameters; the last is
        // optional and ignored.
        let ok = unsafe { GetDiskFreeSpaceExW(path, Some(&mut free), Some(&mut total), None) };
        if ok.is_err() || total == 0 {
            continue;
        }

        let mut label_buffer = [0u16; 64];
        // SAFETY: the length of each slice describes what the API may write.
        let _ =
            unsafe { GetVolumeInformationW(path, Some(&mut label_buffer), None, None, None, None) };

        volumes.push(VolumeInfo {
            mount: root.trim_end_matches('\\').to_string(),
            label: wide_to_string(&label_buffer),
            free_bytes: free,
            total_bytes: total,
        });
    }
    volumes
}

fn split_drive_strings(buffer: &[u16]) -> Vec<String> {
    let mut result = Vec::new();
    let mut start = 0usize;
    for (index, &unit) in buffer.iter().enumerate() {
        if unit == 0 {
            if index > start {
                result.push(String::from_utf16_lossy(&buffer[start..index]));
            }
            start = index + 1;
        }
    }
    result
}

fn wide_to_string(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_drive_strings() {
        let mut buffer: Vec<u16> = Vec::new();
        for drive in ["C:\\", "D:\\"] {
            buffer.extend(drive.encode_utf16());
            buffer.push(0);
        }
        assert_eq!(split_drive_strings(&buffer), vec!["C:\\", "D:\\"]);
    }

    #[test]
    fn finds_at_least_one_fixed_volume() {
        let volumes = volumes();
        assert!(!volumes.is_empty(), "expected a fixed system volume");
        for volume in &volumes {
            assert!(volume.total_bytes > 0);
            assert!(volume.free_bytes <= volume.total_bytes);
            assert!(volume.mount.ends_with(':'));
        }
    }

    #[test]
    fn disk_counters_open_and_report() {
        if let Some(counters) = DiskCounters::open() {
            std::thread::sleep(std::time::Duration::from_millis(60));
            let rates = counters.sample().expect("rates");
            assert!(rates.read_bytes_per_sec >= 0.0);
            assert!((0.0..=100.0).contains(&rates.busy_percent));
        }
    }
}
