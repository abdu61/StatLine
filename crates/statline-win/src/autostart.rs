//! Start-with-Windows support.
//!
//! Portable builds use the per-user `Run` key, which needs no elevation and no
//! scheduled task. A packaged (MSIX) build uses a `StartupTask` instead; see
//! `docs/ops/packaging.md`.
//!
//! Two rules keep this honest:
//! - The command always points at an explicit executable path, never
//!   `current_exe()`. Under `cargo test` that is the test harness, and a `Run`
//!   value pointing at `target\debug\deps\statline_win-*.exe` makes Windows
//!   flash a console window at login and then launch nothing useful.
//! - An existing value that does not match the expected executable is replaced,
//!   so a stale entry from an older build repairs itself.

use std::path::{Path, PathBuf};
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ, RegCloseKey,
    RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows::core::{Error, HRESULT, PCWSTR, Result, w};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: PCWSTR = w!("StatLine");

fn registry_error(status: WIN32_ERROR) -> Error {
    Error::new(HRESULT::from_win32(status.0), "registry operation failed")
}

/// Quote an executable path so a path with spaces still starts correctly.
fn command_for(executable: &Path) -> String {
    format!("\"{}\"", executable.display())
}

/// Read the command currently stored for `StatLine`.
pub fn stored_command() -> Option<String> {
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
        return None;
    }

    let mut buffer = [0u16; 512];
    let mut size = (buffer.len() * std::mem::size_of::<u16>()) as u32;
    // SAFETY: the buffer is `size` bytes and the out-size is valid.
    let result = unsafe {
        RegGetValueW(
            key,
            None,
            VALUE_NAME,
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    // SAFETY: closing the key we opened.
    unsafe {
        let _ = RegCloseKey(key);
    }
    if result.is_err() {
        return None;
    }

    let length = (size as usize / std::mem::size_of::<u16>()).min(buffer.len());
    let text = String::from_utf16_lossy(&buffer[..length]);
    let text = text.trim_matches('\0').trim();
    (!text.is_empty()).then(|| String::from(text))
}

/// Enable or disable starting with Windows for the current user.
///
/// Writes `"<executable>"` (quoted) to the per-user `Run` key.
pub fn set_enabled_for(executable: &Path, enabled: bool) -> Result<()> {
    let mut key = HKEY::default();
    // SAFETY: the key name is a static wide string and `key` is a valid out-param.
    let status =
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, Some(0), KEY_SET_VALUE, &mut key) };
    if status.is_err() {
        return Err(registry_error(status));
    }

    let result = if enabled {
        let command = command_for(executable);
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

/// Enable or disable starting with Windows using this process's executable.
///
/// Only a release build literally named `statline.exe` is allowed. Debug builds
/// are console-subsystem binaries (the terminal flash at login) and test
/// harnesses are named `statline_win-<hash>.exe`; neither must ever reach the
/// `Run` key.
pub fn set_enabled(enabled: bool) -> Result<()> {
    let executable = std::env::current_exe()?;
    // Removing a stale value must always be possible, even from a debug build
    // that is not allowed to create one.
    if !enabled {
        return set_enabled_for(&executable, false);
    }
    let name = executable
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    if !name.eq_ignore_ascii_case("statline.exe") {
        return Err(Error::new(
            HRESULT::from_win32(0x8007_0057), // ERROR_INVALID_PARAMETER
            format!("refusing to register {name} for startup"),
        ));
    }
    if cfg!(debug_assertions) {
        return Err(Error::new(
            HRESULT::from_win32(0x8007_0057),
            "startup is installed from a release build only",
        ));
    }
    set_enabled_for(&executable, enabled)
}

/// Whether the `Run` value currently exists.
pub fn is_enabled() -> bool {
    stored_command().is_some()
}

/// Whether the stored command matches `executable`.
pub fn points_at(executable: &Path) -> bool {
    stored_command()
        .map(|command| command.eq_ignore_ascii_case(&command_for(executable)))
        .unwrap_or(false)
}

/// Rewrite a stale or missing entry so it matches this executable.
///
/// Called at startup when the setting says "on": an older build may have
/// written a debug path, a moved folder, or (in the worst case) a test binary.
/// Returns `Ok(true)` when the value was rewritten.
pub fn repair(executable: &Path) -> Result<bool> {
    if cfg!(debug_assertions) {
        return Ok(false);
    }
    if !is_enabled() {
        return Ok(false);
    }
    if points_at(executable) {
        return Ok(false);
    }
    set_enabled_for(executable, true)?;
    Ok(true)
}

/// Best-effort path of the installed executable, for diagnostics.
pub fn current_executable() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_quotes_the_path() {
        let command = command_for(Path::new(r"C:\Program Files\StatLine\statline.exe"));
        assert_eq!(command, "\"C:\\Program Files\\StatLine\\statline.exe\"");
    }

    #[test]
    fn reporting_state_does_not_panic() {
        let _ = super::is_enabled();
        let _ = super::stored_command();
    }

    #[test]
    fn test_binaries_are_refused() {
        // The harness is `statline_win-<hash>.exe`, not `statline.exe`; writing
        // it would make Windows flash a console at login.
        assert!(super::set_enabled(true).is_err());
    }
}
