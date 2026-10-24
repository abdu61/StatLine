//! Console output for CLI modes.
//!
//! Release builds use the `windows` subsystem, so there is no console until we
//! borrow the parent's. Debug builds already have one; both paths go through
//! [`write_line`] so CLI output behaves the same everywhere.

use std::sync::OnceLock;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE, WriteConsoleW,
};

static ATTEMPTED: OnceLock<()> = OnceLock::new();

fn stdout_handle() -> Option<HANDLE> {
    // SAFETY: no pointers; returns the process's standard output handle.
    let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) }.ok()?;
    if handle.is_invalid() {
        None
    } else {
        Some(handle)
    }
}

/// Attach to the parent process's console if we do not have one. Idempotent.
pub fn ensure() {
    ATTEMPTED.get_or_init(|| {
        if stdout_handle().is_none() {
            // SAFETY: borrows the parent's console; failure is expected and
            // harmless when launched from Explorer.
            unsafe {
                let _ = AttachConsole(ATTACH_PARENT_PROCESS);
            }
        }
    });
}

/// Write one line to the console. Falls back to `stdout` when no console handle
/// exists (e.g. output redirected in a debug build).
pub fn write_line(text: &str) {
    if let Some(handle) = stdout_handle() {
        let mut buffer: Vec<u16> = text.encode_utf16().collect();
        buffer.push(u16::from(b'\r'));
        buffer.push(u16::from(b'\n'));
        let mut written = 0u32;
        // SAFETY: the buffer is valid for `buffer.len()` UTF-16 code units and
        // `written` is a live out-parameter.
        let result = unsafe { WriteConsoleW(handle, &buffer, Some(&mut written), None) };
        if result.is_ok() {
            return;
        }
    }
    println!("{text}");
}
