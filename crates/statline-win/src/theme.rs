//! The Windows app theme.
//!
//! StatLine follows the system light/dark choice only when the user asks
//! (`theme = "system"`). The registry value is the same one Explorer uses, so
//! there is no lag waiting for a theme-change broadcast.

use std::mem::size_of;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::core::{PCWSTR, w};

const PERSONALIZE: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
const APPS_USE_LIGHT_THEME: PCWSTR = w!("AppsUseLightTheme");
const DWM_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\DWM");
const ACCENT_COLOR: PCWSTR = w!("AccentColor");

/// `true` when the Windows app theme is dark.
///
/// Reads `AppsUseLightTheme` (1 = light, 0 = dark). A missing value or a failed
/// read is treated as dark, which is the palette the product is designed around.
pub fn is_dark_mode() -> bool {
    let mut light = 1u32;
    let mut size = size_of::<u32>() as u32;

    // SAFETY: the key/value names are static wide strings; `light` and `size`
    // are valid out-parameters for a DWORD.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PERSONALIZE,
            APPS_USE_LIGHT_THEME,
            RRF_RT_REG_DWORD,
            None,
            Some((&mut light as *mut u32).cast()),
            Some(&mut size),
        )
    };

    if status.is_err() {
        return true;
    }
    light == 0
}

/// Windows accent colour as RGB, when the DWM value is readable.
///
/// `HKCU\Software\Microsoft\Windows\DWM\AccentColor` is stored as `0xAABBGGRR`,
/// so the channels are read red, green, blue from the low bytes.
pub fn accent_rgb() -> Option<(u8, u8, u8)> {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;

    // SAFETY: static key/value names; `value` and `size` are valid out-parameters.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            DWM_KEY,
            ACCENT_COLOR,
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    if status.is_err() || value == 0 {
        return None;
    }

    Some((
        (value & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        ((value >> 16) & 0xFF) as u8,
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn theme_query_does_not_panic() {
        let _ = super::is_dark_mode();
    }

    #[test]
    fn accent_query_does_not_panic() {
        let _ = super::accent_rgb();
    }
}
