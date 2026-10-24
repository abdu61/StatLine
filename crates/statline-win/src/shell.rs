//! Opening things with the shell.

use crate::to_wide;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{PCWSTR, Result, w};

/// Open a filesystem path (folder or file) with its default handler.
pub fn open_path(path: &std::path::Path) -> Result<()> {
    let wide = to_wide(path.as_os_str());
    // SAFETY: `wide` is NUL-terminated and outlives the call; all other
    // arguments are either null or static wide strings.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW returns a value > 32 on success (legacy contract).
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err(windows::core::Error::from_thread())
    }
}
