//! Single-instance guard.
//!
//! A named mutex is the cheapest correct answer on Windows: no lock file, no
//! races on startup, and it is released by the kernel even if the process is
//! killed. The guard is dropped on exit.

use crate::to_wide;
use std::ffi::OsStr;
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, HANDLE, SetLastError, WIN32_ERROR};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::PCWSTR;

/// Holds the mutex for the process lifetime.
pub struct InstanceGuard {
    handle: HANDLE,
}

impl InstanceGuard {
    /// Acquire the named mutex, or return `None` when another instance owns it.
    pub fn acquire(name: &str) -> Option<Self> {
        let wide = to_wide(OsStr::new(name));
        // Clear the thread error first: `CreateMutexW` only sets
        // ERROR_ALREADY_EXISTS when the name exists, and any earlier call in
        // this thread could otherwise leave that value behind.
        // SAFETY: no pointers involved.
        unsafe {
            SetLastError(WIN32_ERROR(0));
        }
        // SAFETY: the name is NUL-terminated and outlives the call; no security
        // descriptor means the default DACL, which is what we want.
        let handle = match unsafe { CreateMutexW(None, false, PCWSTR(wide.as_ptr())) } {
            Ok(handle) => handle,
            Err(error) => {
                log::error!("CreateMutexW failed: {error}");
                return None;
            }
        };

        // SAFETY: reading the thread's last-error value immediately after the
        // call that set it.
        let last = unsafe { windows::Win32::Foundation::GetLastError() };
        if last == ERROR_ALREADY_EXISTS {
            log::info!("single-instance mutex already held");
            // SAFETY: the handle was just created and is not used again.
            unsafe {
                let _ = CloseHandle(handle);
            }
            None
        } else {
            Some(Self { handle })
        }
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this guard and closed exactly once.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_fails_while_the_first_is_alive() {
        let name = format!("Statline.Test.{}", std::process::id());
        let first = InstanceGuard::acquire(&name);
        assert!(first.is_some(), "first acquire must succeed");
        // A second CreateMutexW for the same name yields ERROR_ALREADY_EXISTS.
        assert!(InstanceGuard::acquire(&name).is_none());
        drop(first);
        // After release the name is free again (or at least obtainable).
        assert!(InstanceGuard::acquire(&name).is_some());
    }
}
