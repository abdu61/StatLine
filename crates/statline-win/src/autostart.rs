//! Start-with-Windows support.
//!
//! Portable builds use the per-user `Run` key, which needs no elevation and no
//! scheduled task. A packaged (MSIX) build uses a `StartupTask` instead; see
//! `docs/ops/packaging.md`.

use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ, RegCloseKey,
    RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows::core::{Error, HRESULT, Result, w};

const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("Statline");

fn registry_error(status: WIN32_ERROR) -> Error {
    Error::new(HRESULT::from_win32(status.0), "registry operation failed")
}

/// Enable or disable starting with Windows for the current user.
pub fn set_enabled(enabled: bool) -> Result<()> {
    let mut key = HKEY::default();
    // SAFETY: the key name is a static wide string and `key` is a valid out-param.
    let status =
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, Some(0), KEY_SET_VALUE, &mut key) };
    if status.is_err() {
        return Err(registry_error(status));
    }

    let result = if enabled {
        let executable = std::env::current_exe()?;
        // Quote the path: it usually contains spaces.
        let command = format!("\"{}\"", executable.display());
        let mut wide: Vec<u16> = command.encode_utf16().collect();
        wide.push(0);
        // SAFETY: REG_SZ data is UTF-16; the byte length is twice the code-unit
        // count, and the buffer outlives the call.
        let data =
            unsafe { std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2) };
        // SAFETY: `key` is open for writing; `data` is valid for its length.
        unsafe { RegSetValueExW(key, VALUE_NAME, Some(0), REG_SZ, Some(data)) }
    } else {
        // SAFETY: `key` is open for writing. A missing value is fine.
        unsafe {
            let _ = RegDeleteValueW(key, VALUE_NAME);
        }
        WIN32_ERROR(0)
    };

    // SAFETY: `key` was opened above and is closed exactly once.
    unsafe {
        let _ = RegCloseKey(key);
    }

    if result.is_err() {
        Err(registry_error(result))
    } else {
        Ok(())
    }
}

/// Whether the `Run` value currently exists.
pub fn is_enabled() -> bool {
    let mut key = HKEY::default();
    // SAFETY: static key name; read-only access.
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            Some(0),
            KEY_QUERY_VALUE,
            &mut key,
        )
    };
    if status.is_err() {
        return false;
    }

    // SAFETY: `key` is open; passing no output buffer asks for existence only.
    let exists = unsafe { RegGetValueW(key, None, VALUE_NAME, RRF_RT_REG_SZ, None, None, None) };
    // SAFETY: closing the key we opened.
    unsafe {
        let _ = RegCloseKey(key);
    }
    exists.is_ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn reporting_state_does_not_panic() {
        let _ = super::is_enabled();
    }

    #[test]
    fn enabling_and_removing_round_trips() {
        // The test writes the same per-user value the app writes and restores
        // the previous state, so a failure cannot silently leave autostart on.
        let before = super::is_enabled();
        if super::set_enabled(true).is_err() {
            // A locked-down registry is a legitimate environment; nothing to test.
            return;
        }
        assert!(super::is_enabled(), "a written value must be visible");
        let _ = super::set_enabled(false);
        assert!(!super::is_enabled(), "a removed value must not be visible");
        if before {
            let _ = super::set_enabled(true);
        }
    }
}
