//! Battery state, remaining runtime, design capacity and temperature.

use std::mem::size_of;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, GUID_DEVCLASS_BATTERY, HDEVINFO,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList,
    SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Power::{
    BATTERY_INFORMATION, BATTERY_QUERY_INFORMATION, BATTERY_TAG_INVALID, BatteryInformation,
    BatteryTemperature, CallNtPowerInformation, GetSystemPowerStatus,
    IOCTL_BATTERY_QUERY_INFORMATION, IOCTL_BATTERY_QUERY_TAG, SYSTEM_BATTERY_STATE,
    SYSTEM_POWER_STATUS, SystemBatteryState,
};
use windows::core::PCWSTR;

/// Battery reading, or `None` on a machine without one.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BatteryReading {
    pub percent: f32,
    pub on_ac: bool,
    pub seconds_left: Option<u32>,
    /// Charge (+) or discharge (−) rate in milliwatts.
    pub rate_mw: Option<i32>,
}

pub fn battery() -> Option<BatteryReading> {
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: `status` is a valid out-parameter.
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;

    // BatteryFlag 255 means "no system battery".
    let present = status.BatteryFlag != 255 && status.BatteryLifePercent != 255;
    if !present {
        return None;
    }

    let percent = status.BatteryLifePercent as f32;
    let on_ac = status.ACLineStatus == 1;
    let seconds_left = if status.BatteryLifeTime == u32::MAX || on_ac {
        None
    } else {
        Some(status.BatteryLifeTime)
    };

    Some(BatteryReading {
        percent,
        on_ac,
        seconds_left,
        rate_mw: battery_rate(),
    })
}

/// Health and temperature from the battery device, when it exposes them.
///
/// This opens a device and issues IOCTLs, so callers must cache it; the state
/// changes slowly (design capacity is fixed, temperature moves like a sensor).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BatteryDetails {
    pub designed_mwh: Option<u32>,
    pub full_mwh: Option<u32>,
    pub temperature_c: Option<f32>,
}

/// Handle that closes itself, so every early return releases the device.
struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: the handle was opened here and is closed exactly once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Device set that destroys itself, so every early return cleans up.
struct DeviceSet(HDEVINFO);

impl Drop for DeviceSet {
    fn drop(&mut self) {
        // SAFETY: the set was returned by SetupDiGetClassDevs and is destroyed once.
        unsafe {
            let _ = SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}

pub fn details() -> Option<BatteryDetails> {
    let device = open_battery_device()?;
    let tag = battery_tag(&device)?;

    let query = BATTERY_QUERY_INFORMATION {
        BatteryTag: tag,
        InformationLevel: BatteryInformation,
        AtRate: 0,
    };
    let mut information = BATTERY_INFORMATION::default();
    let mut returned = 0u32;
    // SAFETY: `query` is valid; the output buffer is sized and aligned for
    // BATTERY_INFORMATION, which is what the level asks for.
    unsafe {
        DeviceIoControl(
            device.0,
            IOCTL_BATTERY_QUERY_INFORMATION,
            Some((&query as *const BATTERY_QUERY_INFORMATION).cast()),
            size_of::<BATTERY_QUERY_INFORMATION>() as u32,
            Some((&mut information as *mut BATTERY_INFORMATION).cast()),
            size_of::<BATTERY_INFORMATION>() as u32,
            Some(&mut returned),
            None,
        )
    }
    .ok()?;

    let temperature_c = battery_temperature(&device, tag);

    Some(BatteryDetails {
        designed_mwh: (information.DesignedCapacity > 0).then_some(information.DesignedCapacity),
        full_mwh: (information.FullChargedCapacity > 0).then_some(information.FullChargedCapacity),
        temperature_c,
    })
}

fn battery_temperature(device: &OwnedHandle, tag: u32) -> Option<f32> {
    let query = BATTERY_QUERY_INFORMATION {
        BatteryTag: tag,
        InformationLevel: BatteryTemperature,
        AtRate: 0,
    };
    let mut raw = 0u32;
    let mut returned = 0u32;
    // SAFETY: the level returns one ULONG, in tenths of a Kelvin.
    unsafe {
        DeviceIoControl(
            device.0,
            IOCTL_BATTERY_QUERY_INFORMATION,
            Some((&query as *const BATTERY_QUERY_INFORMATION).cast()),
            size_of::<BATTERY_QUERY_INFORMATION>() as u32,
            Some((&mut raw as *mut u32).cast()),
            size_of::<u32>() as u32,
            Some(&mut returned),
            None,
        )
    }
    .ok()?;

    let celsius = raw as f32 / 10.0 - 273.15;
    (-20.0..=80.0).contains(&celsius).then_some(celsius)
}

fn battery_tag(device: &OwnedHandle) -> Option<u32> {
    let mut tag = 0u32;
    let mut returned = 0u32;
    // SAFETY: no input buffer; one ULONG is written.
    unsafe {
        DeviceIoControl(
            device.0,
            IOCTL_BATTERY_QUERY_TAG,
            None,
            0,
            Some((&mut tag as *mut u32).cast()),
            size_of::<u32>() as u32,
            Some(&mut returned),
            None,
        )
    }
    .ok()?;
    (tag != BATTERY_TAG_INVALID).then_some(tag)
}

/// Open the first battery device interface.
fn open_battery_device() -> Option<OwnedHandle> {
    // SAFETY: the class GUID is static; no enumerator or parent window is used.
    let set = unsafe {
        SetupDiGetClassDevsW(
            Some(&GUID_DEVCLASS_BATTERY),
            PCWSTR::null(),
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    }
    .ok()?;
    let set = DeviceSet(set);

    let mut interface = SP_DEVICE_INTERFACE_DATA {
        cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
        ..Default::default()
    };
    // SAFETY: the set is live; member 0 of the battery class is requested.
    unsafe { SetupDiEnumDeviceInterfaces(set.0, None, &GUID_DEVCLASS_BATTERY, 0, &mut interface) }
        .ok()?;

    let mut required = 0u32;
    // SAFETY: a null detail buffer asks for the required size; the returned
    // ERROR_INSUFFICIENT_BUFFER is expected and the size is written.
    unsafe {
        let _ =
            SetupDiGetDeviceInterfaceDetailW(set.0, &interface, None, 0, Some(&mut required), None);
    }
    if required == 0 {
        return None;
    }

    // u64 words keep the variable-length detail structure aligned.
    let mut buffer = vec![0u64; (required as usize).div_ceil(size_of::<u64>())];
    let detail = buffer
        .as_mut_ptr()
        .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
    // SAFETY: `detail` points at the start of the aligned buffer; cbSize must
    // name the structure's own size before the call.
    unsafe {
        (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
    }
    // SAFETY: the buffer holds `required` bytes, which the previous call asked for.
    unsafe {
        SetupDiGetDeviceInterfaceDetailW(set.0, &interface, Some(detail), required, None, None)
    }
    .ok()?;

    // SAFETY: the call succeeded and wrote the NUL-terminated device path.
    let path = unsafe { (*detail).DevicePath.as_ptr() };
    // SAFETY: `path` is a valid NUL-terminated device interface path.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(path),
            GENERIC_READ.0 | GENERIC_WRITE.0,
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

fn battery_rate() -> Option<i32> {
    let mut state = SYSTEM_BATTERY_STATE::default();
    let bytes = size_of::<SYSTEM_BATTERY_STATE>() as u32;

    // SAFETY: no input buffer; the output buffer is correctly sized and aligned.
    let status = unsafe {
        CallNtPowerInformation(
            SystemBatteryState,
            None,
            0,
            Some((&mut state as *mut SYSTEM_BATTERY_STATE).cast()),
            bytes,
        )
    };
    if status.0 < 0 || !state.BatteryPresent {
        return None;
    }

    // `Rate` is milliwatts; discharging is negative on most systems.
    Some(state.Rate as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_percent_is_in_range_when_present() {
        if let Some(battery) = battery() {
            assert!((0.0..=100.0).contains(&battery.percent));
        }
    }

    #[test]
    fn details_are_plausible_when_present() {
        if let Some(details) = details() {
            if let Some(design) = details.designed_mwh {
                assert!(design > 0);
            }
            if let Some(full) = details.full_mwh {
                assert!(full > 0);
            }
            if let Some(temperature) = details.temperature_c {
                assert!((-20.0..=80.0).contains(&temperature));
            }
        }
    }
}
