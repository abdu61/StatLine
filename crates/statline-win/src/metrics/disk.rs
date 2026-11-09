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
    IOCTL_STORAGE_PREDICT_FAILURE, IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery,
    STORAGE_PROPERTY_QUERY, STORAGE_TEMPERATURE_DATA_DESCRIPTOR,
    STORAGE_TEMPERATURE_VALUE_NOT_REPORTED, StorageDeviceProperty,
    StorageDeviceTemperatureProperty, VOLUME_DISK_EXTENTS,
};
use windows::core::PCWSTR;

/// `DRIVE_FIXED` from winbase.h; the constant is not generated in this binding.
const DRIVE_FIXED: u32 = 3;

/// Throughput rates for one physical disk.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiskActivity {
    /// PDH instance name, e.g. `0 C:`.
    pub name: String,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    pub busy_percent: f32,
}

/// Throughput rates for the aggregate plus each physical disk.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiskRates {
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    pub busy_percent: f32,
    pub disks: Vec<DiskActivity>,
}

/// One open PDH query holding every disk counter we need.
pub struct DiskCounters {
    query: PdhQuery,
    read: PdhCounter,
    write: PdhCounter,
    busy: PdhCounter,
}

impl DiskCounters {
    /// Open the query and add the three wildcard counters.
    pub fn open() -> Option<Self> {
        let query = PdhQuery::open()?;
        let read = query.add_english("\\PhysicalDisk(*)\\Disk Read Bytes/sec")?;
        let write = query.add_english("\\PhysicalDisk(*)\\Disk Write Bytes/sec")?;
        let busy = query.add_english("\\PhysicalDisk(*)\\% Disk Time")?;
        // Prime the rate counters; the first sample has no interval.
        let _ = query.collect();
        Some(Self {
            query,
            read,
            write,
            busy,
        })
    }

    /// Refresh and read the aggregate plus per-disk rates.
    pub fn sample(&self) -> Option<DiskRates> {
        if !self.query.collect() {
            return None;
        }

        let reads = self.read.named_values()?;
        let writes = self.write.named_values().unwrap_or_default();
        let busy = self.busy.named_values().unwrap_or_default();

        let mut rows: HashMap<String, DiskActivity> = HashMap::new();
        for (name, value) in reads {
            rows.entry(name.clone())
                .or_insert_with(|| DiskActivity {
                    name,
                    ..DiskActivity::default()
                })
                .read_bytes_per_sec = value.max(0.0);
        }
        for (name, value) in writes {
            rows.entry(name.clone())
                .or_insert_with(|| DiskActivity {
                    name,
                    ..DiskActivity::default()
                })
                .write_bytes_per_sec = value.max(0.0);
        }
        for (name, value) in busy {
            rows.entry(name.clone())
                .or_insert_with(|| DiskActivity {
                    name,
                    ..DiskActivity::default()
                })
                .busy_percent = value.clamp(0.0, 100.0) as f32;
        }

        let total = rows.remove("_Total");
        let mut disks: Vec<DiskActivity> = rows.into_values().collect();
        disks.sort_by(|a, b| a.name.cmp(&b.name));

        let aggregate = total.unwrap_or_else(|| DiskActivity {
            name: String::from("_Total"),
            read_bytes_per_sec: disks.iter().map(|disk| disk.read_bytes_per_sec).sum(),
            write_bytes_per_sec: disks.iter().map(|disk| disk.write_bytes_per_sec).sum(),
            busy_percent: disks
                .iter()
                .map(|disk| disk.busy_percent)
                .sum::<f32>()
                .clamp(0.0, 100.0),
        });

        Some(DiskRates {
            read_bytes_per_sec: aggregate.read_bytes_per_sec,
            write_bytes_per_sec: aggregate.write_bytes_per_sec,
            busy_percent: aggregate.busy_percent,
            disks,
        })
    }
}

/// Static identity and health of one physical disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiskHealth {
    /// Model string from the device descriptor, e.g. `Samsung SSD 980`.
    pub model: String,
    /// Bus type name (`NVMe`, `SATA`, `USB`, …).
    pub bus: String,
    /// Disk size in bytes.
    pub size_bytes: u64,
    /// `true`/`false` from the device's predictive-failure bit; `None` when the
    /// drive does not implement `IOCTL_STORAGE_PREDICT_FAILURE`.
    pub healthy: Option<bool>,
}

/// Identity and predictive-failure state per physical disk number.
///
/// The descriptor query works on a zero-access handle, so the model and bus
/// need no elevation. Two things deliberately do **not** come from the raw
/// device:
///
/// - **Capacity** is summed from the volumes on that disk. `IOCTL_DISK_GET_
///   LENGTH_INFO` and `IOCTL_STORAGE_PREDICT_FAILURE` both require
///   administrator access to `\\.\PhysicalDriveN`, and this app never asks for
///   elevation.
/// - **Health** is therefore `None` unless the drive reports it without
///   elevation. A missing value is honest; a fabricated "OK" is not.
///
/// The data is static, so callers should read it once.
pub fn disk_health() -> Vec<(u32, DiskHealth)> {
    let capacities = capacity_by_disk();
    let mut result = Vec::new();
    for disk in 0..16u32 {
        let Some(device) = open_device(&format!("\\\\.\\PhysicalDrive{disk}")) else {
            continue;
        };
        let Some((model, bus)) = storage_identity(device.0) else {
            continue;
        };
        let health = DiskHealth {
            model,
            bus,
            size_bytes: capacities.get(&disk).copied().unwrap_or(0),
            healthy: predictive_failure(device.0),
        };
        result.push((disk, health));
    }
    result
}

/// Total size of the mounted volumes on each physical disk.
///
/// Volumes are visible without elevation, so this is the one capacity path a
/// standard user process can take.
fn capacity_by_disk() -> HashMap<u32, u64> {
    let mut buffer = vec![0u16; 512];
    // SAFETY: valid writable buffer; the return is the character count.
    let written = unsafe { GetLogicalDriveStringsW(Some(&mut buffer)) };
    if written == 0 || written as usize > buffer.len() {
        return HashMap::new();
    }

    let mut totals: HashMap<u32, u64> = HashMap::new();
    for root in split_drive_strings(&buffer[..written as usize]) {
        let wide = to_wide(OsStr::new(&root));
        // SAFETY: `path` is NUL-terminated and outlives the call.
        if unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) } != DRIVE_FIXED {
            continue;
        }
        let mut total = 0u64;
        // SAFETY: valid out-parameters; the last is optional and ignored.
        let ok =
            unsafe { GetDiskFreeSpaceExW(PCWSTR(wide.as_ptr()), None, Some(&mut total), None) };
        if ok.is_err() || total == 0 {
            continue;
        }
        if let Some(disk) = volume_disk_number(&root) {
            *totals.entry(disk).or_default() += total;
        }
    }
    totals
}

/// Model string and bus name from the device descriptor.
///
/// `STORAGE_DEVICE_DESCRIPTOR` stores strings as byte offsets into the same
/// buffer, not inline arrays.
fn storage_identity(device: HANDLE) -> Option<(String, String)> {
    use windows::Win32::System::Ioctl::STORAGE_DEVICE_DESCRIPTOR;

    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut buffer = [0u64; 128];
    let mut returned = 0u32;
    // SAFETY: `query` is a valid descriptor; the output buffer is 1 KiB and
    // aligned, and the IOCTL writes a variable-length descriptor into it.
    let result = unsafe {
        DeviceIoControl(
            device,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some((&query as *const STORAGE_PROPERTY_QUERY).cast()),
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            Some(buffer.as_mut_ptr().cast()),
            (buffer.len() * size_of::<u64>()) as u32,
            Some(&mut returned),
            None,
        )
    };
    if result.is_err() || returned < size_of::<STORAGE_DEVICE_DESCRIPTOR>() as u32 {
        return None;
    }

    // SAFETY: the header fits in `returned`; its offset fields point inside the
    // same buffer, which is `returned` bytes long.
    let descriptor = unsafe { &*(buffer.as_ptr().cast::<STORAGE_DEVICE_DESCRIPTOR>()) };
    let bytes =
        unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), returned as usize) };
    let field = |offset: u32| {
        if offset == 0 {
            String::new()
        } else {
            c_string_at(bytes, offset as usize)
        }
    };
    let model = field(descriptor.ProductIdOffset);
    let vendor = field(descriptor.VendorIdOffset);

    // Firmware often stores the model alone; a vendor prefix only helps when
    // the model does not already name it.
    let model = if model.is_empty() {
        vendor
    } else if !vendor.is_empty() && !model.to_lowercase().contains(&vendor.to_lowercase()) {
        format!("{vendor} {model}")
    } else {
        model
    };

    Some((model.trim().to_string(), bus_type_name(descriptor.BusType)))
}

/// NUL-terminated ASCII string at `offset`.
fn c_string_at(bytes: &[u8], offset: usize) -> String {
    let tail = match bytes.get(offset..) {
        Some(tail) => tail,
        None => return String::new(),
    };
    let end = tail
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(tail.len());
    String::from_utf8_lossy(&tail[..end]).trim().to_string()
}

/// `IOCTL_STORAGE_PREDICT_FAILURE`: the drive's own "about to fail" bit.
fn predictive_failure(device: HANDLE) -> Option<bool> {
    let mut buffer = [0u64; 16];
    let mut returned = 0u32;
    // SAFETY: no input buffer; a 128-byte aligned output buffer, and the IOCTL
    // reports how much it wrote.
    let result = unsafe {
        DeviceIoControl(
            device,
            IOCTL_STORAGE_PREDICT_FAILURE,
            None,
            0,
            Some(buffer.as_mut_ptr().cast()),
            (buffer.len() * size_of::<u64>()) as u32,
            Some(&mut returned),
            None,
        )
    };
    result.as_ref().ok()?;
    let _ = returned;
    // SAFETY: STORAGE_PREDICT_FAILURE starts with a DWORD PredictFailure flag;
    // the buffer is aligned for it.
    let flags = unsafe { *buffer.as_ptr().cast::<u32>() };
    Some(flags & 1 == 0)
}

/// Human name for `STORAGE_BUS_TYPE`.
fn bus_type_name(bus: windows::Win32::Storage::FileSystem::STORAGE_BUS_TYPE) -> String {
    use windows::Win32::Storage::FileSystem::STORAGE_BUS_TYPE;
    let name = match bus {
        STORAGE_BUS_TYPE(0x03) => "ATA",
        STORAGE_BUS_TYPE(0x07) => "USB",
        STORAGE_BUS_TYPE(0x08) => "RAID",
        STORAGE_BUS_TYPE(0x09) => "iSCSI",
        STORAGE_BUS_TYPE(0x0A) => "SAS",
        STORAGE_BUS_TYPE(0x0B) => "SATA",
        STORAGE_BUS_TYPE(0x0C) => "SD",
        STORAGE_BUS_TYPE(0x0D) => "MMC",
        STORAGE_BUS_TYPE(0x11) => "NVMe",
        STORAGE_BUS_TYPE(0x12) => "SCM",
        _ => "",
    };
    String::from(name)
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
    open_device_with(path, 0)
}

/// Open a device path requesting `access`.
fn open_device_with(path: &str, access: u32) -> Option<OwnedHandle> {
    let wide = to_wide(OsStr::new(path));
    // SAFETY: the path is NUL-terminated and outlives the call; no security
    // descriptor or template is needed.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            access,
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

    /// A virtual disk on a CI runner may expose no descriptor at all, and its
    /// bus type can be one this binding does not name. Assert the shape of what
    /// is offered instead of requiring a physical machine.
    #[test]
    fn disk_health_reports_identity_when_available() {
        for (_, health) in disk_health() {
            assert!(
                !health.model.is_empty() || health.bus.is_empty(),
                "a model without a bus type is a parse bug: {health:?}"
            );
            if !health.bus.is_empty() {
                assert!(
                    [
                        "NVMe", "SATA", "ATA", "USB", "RAID", "SAS", "iSCSI", "SD", "MMC", "SCM"
                    ]
                    .contains(&&*health.bus),
                    "unexpected bus type {:?}",
                    health.bus
                );
            }
        }
    }

    #[test]
    fn disk_counters_open_and_report() {
        if let Some(counters) = DiskCounters::open() {
            std::thread::sleep(std::time::Duration::from_millis(60));
            // The query can open yet report nothing on a machine with no
            // physical-disk instances.
            if let Some(rates) = counters.sample() {
                assert!(rates.read_bytes_per_sec >= 0.0);
                assert!((0.0..=100.0).contains(&rates.busy_percent));
            }
        }
    }
}
