//! Static platform identity: Windows edition/build and computer name.
//!
//! Read once at startup from the registry; no WMI, no elevation.

use std::mem::size_of;
use windows::Win32::System::Registry::{
    HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
};
use windows::core::{PCWSTR, w};

const WINDOWS_KEY: PCWSTR = w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
const PRODUCT_NAME: PCWSTR = w!("ProductName");
const DISPLAY_VERSION: PCWSTR = w!("DisplayVersion");
const BUILD_NUMBER: PCWSTR = w!("CurrentBuildNumber");
const UBR: PCWSTR = w!("UBR");

const COMPUTER_KEY: PCWSTR = w!("SYSTEM\\CurrentControlSet\\Control\\ComputerName\\ComputerName");
const COMPUTER_NAME: PCWSTR = w!("ComputerName");

/// Human-readable OS and computer name, when the registry exposes them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlatformInfo {
    pub os: Option<String>,
    pub computer: Option<String>,
}

impl PlatformInfo {
    /// Read the current values. Missing keys simply produce `None`.
    pub fn read() -> Self {
        Self {
            os: os_string(),
            computer: string_value(COMPUTER_KEY, COMPUTER_NAME),
        }
    }
}

fn os_string() -> Option<String> {
    let mut name = string_value(WINDOWS_KEY, PRODUCT_NAME)?;
    let edition = string_value(WINDOWS_KEY, DISPLAY_VERSION);
    let build = string_value(WINDOWS_KEY, BUILD_NUMBER).and_then(|value| value.parse::<u32>().ok());
    let ubr = dword_value(WINDOWS_KEY, UBR);

    // Windows 11 still reports "Windows 10" in ProductName on many builds.
    if build.is_some_and(|build| build >= 22_000) {
        name = name.replace("Windows 10", "Windows 11");
    }

    let mut text = name;
    if let Some(edition) = edition.filter(|edition| !edition.is_empty()) {
        text.push(' ');
        text.push_str(&edition);
    }
    if let Some(build) = build {
        text.push_str(&format!(" (build {build}"));
        if let Some(ubr) = ubr {
            text.push_str(&format!(".{ubr}"));
        }
        text.push(')');
    }
    Some(text)
}

fn string_value(key: PCWSTR, value: PCWSTR) -> Option<String> {
    let mut buffer = [0u16; 256];
    let mut size = (buffer.len() * size_of::<u16>()) as u32;

    // SAFETY: static key/value names; the buffer is `size` bytes and the output
    // size is a valid out-parameter.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key,
            value,
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
    let text = String::from_utf16_lossy(&buffer[..length]);
    let text = text.trim_matches('\0').trim();
    (!text.is_empty()).then(|| String::from(text))
}

fn dword_value(key: PCWSTR, value: PCWSTR) -> Option<u32> {
    let mut data = 0u32;
    let mut size = size_of::<u32>() as u32;

    // SAFETY: static key/value names; `data` and `size` are valid DWORD
    // out-parameters.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key,
            value,
            RRF_RT_REG_DWORD,
            None,
            Some((&mut data as *mut u32).cast()),
            Some(&mut size),
        )
    };
    status.is_ok().then_some(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_identity_is_plausible() {
        let info = PlatformInfo::read();
        if let Some(os) = info.os {
            assert!(os.contains("Windows"), "unexpected OS string: {os}");
        }
        if let Some(computer) = info.computer {
            assert!(!computer.is_empty());
        }
    }
}
