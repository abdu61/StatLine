//! Network interface counters.
//!
//! `GetIfTable2` costs ~1.2 ms on the reference machine because it walks every
//! interface and adapter in the system. Sampling once per second with it blows
//! the budget, so the shape is:
//!
//! - [`interfaces`] enumerates once every few seconds and caches LUIDs.
//! - [`counters`] reads a single interface per tick with `GetIfEntry2` (tens of
//!   microseconds), so the per-tick cost stays small.

use std::ffi::c_void;
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfEntry2, GetIfTable2, MIB_IF_ROW2, MIB_IF_TABLE2,
};
use windows::Win32::NetworkManagement::Ndis::NET_LUID_LH;

/// A cached, named interface, cheap to re-read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceRef {
    pub name: String,
    /// Raw `NET_LUID_LH.Value`, kept as an integer so the struct stays simple.
    pub luid: u64,
    /// Negotiated link speed in bits per second.
    pub link_speed_bps: u64,
    /// IEEE 802.11 interface.
    pub wireless: bool,
}

const IF_OPER_STATUS_UP: i32 = 1;
const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
const IF_TYPE_IEEE80211: u32 = 71;
const TUNNEL_TYPE_NONE: i32 = 0;

/// Enumerate operational, non-loopback, non-tunnel interfaces with a real link.
pub fn interfaces() -> Vec<InterfaceRef> {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    // SAFETY: `table` is a valid out-parameter; the API allocates the table.
    let error = unsafe { GetIfTable2(&mut table) };
    if error.0 != 0 || table.is_null() {
        return Vec::new();
    }

    // SAFETY: on success the API returns a table with `NumEntries` rows, laid
    // out contiguously starting at `Table`.
    let result = unsafe {
        let count = (*table).NumEntries as usize;
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), count);
        let mut found = Vec::new();
        for row in rows {
            if row.OperStatus.0 != IF_OPER_STATUS_UP
                || row.Type == IF_TYPE_SOFTWARE_LOOPBACK
                || row.TunnelType.0 != TUNNEL_TYPE_NONE
            {
                continue;
            }
            // Disconnected virtual adapters report no link speed.
            if row.ReceiveLinkSpeed == 0 && row.TransmitLinkSpeed == 0 {
                continue;
            }
            // The LUID union is initialized by the API; read its raw value.
            let luid = row.InterfaceLuid.Value;
            found.push(InterfaceRef {
                name: wide_to_string(&row.Alias),
                luid,
                link_speed_bps: row.TransmitLinkSpeed.max(row.ReceiveLinkSpeed),
                wireless: row.Type == IF_TYPE_IEEE80211,
            });
        }
        found
    };

    // SAFETY: the table came from GetIfTable2 and is freed exactly once.
    unsafe { FreeMibTable(table as *const c_void) };
    result
}

/// Current octet totals for one interface.
pub fn counters(interface: &InterfaceRef) -> Option<(u64, u64)> {
    let mut row = MIB_IF_ROW2 {
        InterfaceLuid: NET_LUID_LH {
            Value: interface.luid,
        },
        ..Default::default()
    };

    // SAFETY: `row` is zeroed except for the LUID, which is what GetIfEntry2
    // uses to identify the interface.
    let error = unsafe { GetIfEntry2(&mut row) };
    if error.0 != 0 {
        return None;
    }
    Some((row.InOctets, row.OutOctets))
}

fn wide_to_string(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_names_are_trimmed_at_the_nul() {
        let mut buffer = [0u16; 8];
        for (index, unit) in "Wi-Fi".encode_utf16().enumerate() {
            buffer[index] = unit;
        }
        assert_eq!(wide_to_string(&buffer), "Wi-Fi");
    }

    #[test]
    fn enumerates_interfaces_and_reads_their_counters() {
        // Loopback is filtered out, so an offline machine may legitimately
        // return nothing; the contract is "no panic".
        for interface in interfaces() {
            assert!(!interface.name.is_empty());
            if let Some((rx, tx)) = counters(&interface) {
                let _ = (rx, tx);
            }
        }
    }
}
