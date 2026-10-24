//! Disk throughput (PDH) and volume capacity.

use crate::metrics::pdh::{PdhCounter, PdhQuery};
use crate::to_wide;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::mem::size_of;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, GetDiskFreeSpaceExW,
    GetDriveTypeW, GetLogicalDriveStringsW, GetVolumeInformationW,
    IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{
    IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery, STORAGE_PROPERTY_QUERY,
    STORAGE_TEMPERATURE_DATA_DESCRIPTOR, STORAGE_TEMPERATURE_VALUE_NOT_REPORTED,
    StorageDeviceTemperatureProperty, VOLUME_DISK_EXTENTS,
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

/// Temperature per mounted fixed volume.
///
/// Each entry costs a volume IOCTL plus a physical-disk IOCTL, which measured
/// ~11 ms on an NVMe system: callers must cache this for minutes, not seconds.
pub fn volume_temperatures() -> Vec<(String, Option<f32>)> {
    let mut buffer = vec![0u16; 512];
    // SAFETY: valid writable buffer; the return is the character count.
    let written = unsafe { GetLogicalDriveStringsW(Some(&mut buffer)) };
    if written == 0 || written as usize > buffer.len() {
        return Vec::new();
    }

    // A machine can have several volumes on one disk; query its sensor once.
    let mut by_disk: HashMap<u32, Option<f32>> = HashMap::new();
    let mut result = Vec::new();
    for root in split_drive_strings(&buffer[..written as usize]) {
        let wide = to_wide(OsStr::new(&root));
        // SAFETY: `path` is NUL-terminated and outlives the call.
        if unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) } != DRIVE_FIXED {
            continue;
        }

        let temperature = volume_disk_number(&root).and_then(|disk| {
            *by_disk
                .entry(disk)
                .or_insert_with(|| disk_temperature(disk))
        });
        result.push((root.trim_end_matches('\\').to_string(), temperature));
    }
    result
}

/// Handle that closes itself, so every early return releases the device.
struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by `open_device` and is closed once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Open a device path with no access requested; attributes are enough for
/// these IOCTLs, which avoids requiring read access to raw disks.
fn open_device(path: &str) -> Option<OwnedHandle> {
    let wide = to_wide(OsStr::new(path));
    // SAFETY: the path is NUL-terminated and outlives the call; no security
    // descriptor or template is needed.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .ok()?;
    (!handle.is_invalid()).then_some(OwnedHandle(handle))
}

/// Physical disk number backing a mounted volume, e.g. `C:\` → `0`.
fn volume_disk_number(mount: &str) -> Option<u32> {
    let path = format!("\\\\.\\{}", mount.trim_end_matches('\\'));
    let device = open_device(&path)?;

    // u64 words keep the buffer 8-byte aligned for the extents structure.
    let mut buffer = [0u64; 32];
    let mut returned = 0u32;
    // SAFETY: no input buffer; the output buffer is 256 bytes and aligned; the
    // IOCTL reports the number of extents and fills at least one.
    let result = unsafe {
        DeviceIoControl(
            device.0,
            IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
            None,
            0,
            Some(buffer.as_mut_ptr().cast()),
            (buffer.len() * size_of::<u64>()) as u32,
            Some(&mut returned),
            None,
        )
    };
    if result.is_err() || returned < size_of::<VOLUME_DISK_EXTENTS>() as u32 {
        return None;
    }

    // SAFETY: the IOCTL wrote a VOLUME_DISK_EXTENTS whose header fits in
    // `returned`, and the buffer can hold the declared extents.
    let extents = unsafe { &*(buffer.as_ptr().cast::<VOLUME_DISK_EXTENTS>()) };
    (extents.NumberOfDiskExtents > 0).then_some(extents.Extents[0].DiskNumber)
}

/// `StorageDeviceTemperatureProperty` for a physical disk, in °C.
fn disk_temperature(disk: u32) -> Option<f32> {
    let device = open_device(&format!("\\\\.\\PhysicalDrive{disk}"))?;

    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceTemperatureProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut buffer = [0u64; 64];
    let mut returned = 0u32;
    // SAFETY: `query` is a valid descriptor; the output buffer is 512 bytes and
    // aligned, and the IOCTL writes a variable-length descriptor into it.
    let result = unsafe {
        DeviceIoControl(
            device.0,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some((&query as *const STORAGE_PROPERTY_QUERY).cast()),
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            Some(buffer.as_mut_ptr().cast()),
            (buffer.len() * size_of::<u64>()) as u32,
            Some(&mut returned),
            None,
        )
    };
    if result.is_err() || returned < size_of::<STORAGE_TEMPERATURE_DATA_DESCRIPTOR>() as u32 {
        return None;
    }

    // SAFETY: the descriptor header fits in `returned`; `InfoCount` says whether
    // the trailing temperature entry is present.
    let descriptor = unsafe {
        &*(buffer
            .as_ptr()
            .cast::<STORAGE_TEMPERATURE_DATA_DESCRIPTOR>())
    };
    if descriptor.InfoCount == 0 {
        return None;
    }
    let raw = descriptor.TemperatureInfo[0].Temperature;
    if raw as u32 == STORAGE_TEMPERATURE_VALUE_NOT_REPORTED {
        return None;
    }

    let celsius = raw as f32;
    (-20.0..=125.0).contains(&celsius).then_some(celsius)
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
