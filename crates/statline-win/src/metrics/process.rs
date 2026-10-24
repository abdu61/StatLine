//! The system process table: PID, image name, CPU time and working set.
//!
//! `NtQuerySystemInformation(SystemProcessInformation)` returns a linked list of
//! variable-length entries in one buffer. The buffer is owned by the caller and
//! reused across samples, because this query allocates in the kernel.

use std::ffi::c_void;
use std::mem::size_of;
use std::sync::Once;
use windows::Wdk::System::SystemInformation::{NtQuerySystemInformation, SystemProcessInformation};
use windows::Win32::Foundation::UNICODE_STRING;
use windows::Win32::System::Threading::GetCurrentProcessId;

const STATUS_INFO_LENGTH_MISMATCH: u32 = 0xC000_0004;
const INITIAL_BYTES: usize = 256 * 1024;
const MAX_BYTES: usize = 16 * 1024 * 1024;
/// Windows image names are far shorter; this also bounds a corrupt length.
const MAX_NAME_UNITS: usize = 260;

/// One process as reported by the kernel, in 100 ns CPU ticks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawProcess {
    pub pid: u32,
    pub name: String,
    /// User + kernel time consumed since the process started.
    pub cpu_time_100ns: u64,
    pub working_set_bytes: u64,
}

/// The documented prefix of `SYSTEM_PROCESS_INFORMATION`.
///
/// The windows-rs binding replaces the CPU-time fields with `Reserved1`, so the
/// layout is spelled out here. `repr(C)` gives the same padding the kernel uses
/// on every supported target, which keeps the field offsets honest.
#[repr(C)]
struct ProcessInformation {
    next_entry_offset: u32,
    number_of_threads: u32,
    working_set_private_size: i64,
    hard_fault_count: u32,
    number_of_threads_high_watermark: u32,
    cycle_time: u64,
    create_time: i64,
    user_time: i64,
    kernel_time: i64,
    image_name: UNICODE_STRING,
    base_priority: i32,
    unique_process_id: usize,
    inherited_from: usize,
    handle_count: u32,
    session_id: u32,
    unique_process_key: usize,
    peak_virtual_size: usize,
    virtual_size: usize,
    reserved: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
}

static WARNED: Once = Once::new();

/// Fill `out` with the current process table, reusing `buffer` for the query.
///
/// Returns `false` when the kernel refuses the query; the caller keeps showing
/// the previous table. Processes without an image name (Idle, System) are
/// skipped: they have no per-app meaning.
pub fn processes(buffer: &mut Vec<u64>, out: &mut Vec<RawProcess>) -> bool {
    out.clear();

    let mut bytes = INITIAL_BYTES;
    for _ in 0..3 {
        if buffer.len() * size_of::<u64>() < bytes {
            buffer.resize(bytes.div_ceil(size_of::<u64>()), 0);
        }

        let mut needed = 0u32;
        // SAFETY: the buffer can hold at least `bytes` bytes starting at an
        // 8-byte-aligned pointer; the kernel writes at most `bytes` and reports
        // its requirement in `needed`.
        let status = unsafe {
            NtQuerySystemInformation(
                SystemProcessInformation,
                buffer.as_mut_ptr().cast::<c_void>(),
                bytes as u32,
                &mut needed,
            )
        };

        if status.0 >= 0 {
            parse(buffer.as_slice(), out);
            return true;
        }
        if status.0 as u32 != STATUS_INFO_LENGTH_MISMATCH {
            WARNED.call_once(|| {
                log::warn!("process table query failed with status 0x{:08X}", status.0);
            });
            return false;
        }
        // The kernel reports the size it wants; leave headroom for processes
        // that start between the two calls.
        let wanted = needed as usize;
        if wanted == 0 || wanted > MAX_BYTES || wanted <= bytes {
            return false;
        }
        bytes = wanted + 64 * 1024;
    }
    false
}

/// PID of the calling process, for tests and for marking our own row.
pub fn current_pid() -> u32 {
    // SAFETY: no arguments, no pointers.
    unsafe { GetCurrentProcessId() }
}

/// Walk the linked list the kernel wrote into `buffer`.
fn parse(buffer: &[u64], out: &mut Vec<RawProcess>) {
    let base = buffer.as_ptr().cast::<u8>();
    let total_bytes = std::mem::size_of_val(buffer);
    let mut offset = 0usize;

    loop {
        if offset + size_of::<ProcessInformation>() > total_bytes {
            return;
        }

        // SAFETY: `offset` is inside the buffer and leaves room for the whole
        // prefix. Entries are 8-byte aligned by the kernel, but `read_unaligned`
        // keeps this sound even if a future build changes that.
        let entry = unsafe {
            base.add(offset)
                .cast::<ProcessInformation>()
                .read_unaligned()
        };

        let name = image_name(&entry.image_name);
        if !name.is_empty() {
            let cpu_time_100ns = entry.user_time.max(0) as u64 + entry.kernel_time.max(0) as u64;
            out.push(RawProcess {
                pid: entry.unique_process_id as u32,
                name,
                cpu_time_100ns,
                working_set_bytes: entry.working_set_size as u64,
            });
        }

        let next = entry.next_entry_offset as usize;
        if next == 0 {
            return;
        }
        offset = match offset.checked_add(next) {
            Some(next_offset) => next_offset,
            None => return,
        };
    }
}

fn image_name(name: &UNICODE_STRING) -> String {
    let units = (name.Length as usize / 2).min(MAX_NAME_UNITS);
    if units == 0 || name.Buffer.is_null() {
        return String::new();
    }

    // SAFETY: the kernel reports `Length` bytes of UTF-16 at `Buffer`, which
    // points into the buffer this walk is reading, and the slice is copied into
    // an owned string before that buffer is reused.
    let slice = unsafe { std::slice::from_raw_parts(name.Buffer.0, units) };
    String::from_utf16_lossy(slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_returns_a_plausible_process_table() {
        let mut buffer = Vec::new();
        let mut out = Vec::new();
        assert!(processes(&mut buffer, &mut out));
        assert!(out.len() > 4, "expected a populated process table");

        let me = current_pid();
        assert!(
            out.iter().any(|entry| entry.pid == me),
            "the calling process must be in the table"
        );
        assert!(out.iter().all(|entry| !entry.name.is_empty()));
        assert!(out.iter().any(|entry| entry.working_set_bytes > 0));
    }

    #[test]
    fn buffer_is_reused_and_output_is_cleared() {
        let mut buffer = Vec::new();
        let mut out = vec![RawProcess::default()];
        assert!(processes(&mut buffer, &mut out));
        assert!(out.len() > 1);
        assert!(buffer.capacity() >= INITIAL_BYTES / size_of::<u64>());
    }
}
