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
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfEntry2, GetIfTable2, MIB_IF_ROW2, MIB_IF_TABLE2,
};
use windows::Win32::NetworkManagement::Ndis::NET_LUID_LH;
use windows::Win32::NetworkManagement::WiFi::{
    WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO_LIST, WlanCloseHandle, WlanEnumInterfaces,
    WlanFreeMemory, WlanOpenHandle, WlanQueryInterface, wlan_interface_state_connected,
    wlan_intf_opcode_current_connection,
};

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
/// `MIB_IF_ROW2_0` bit 1: an NDIS filter miniport.
///
/// These (`Native WiFi Filter Driver`, NPCAP, QoS Packet Scheduler, WFP…) share
/// the physical adapter's counters, so including them both floods the UI with
/// duplicate rows and multiplies the aggregate rate.
const FILTER_INTERFACE: u8 = 0x02;

fn is_filter_miniport(flags: u8) -> bool {
    flags & FILTER_INTERFACE != 0
}

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
                || is_filter_miniport(row.InterfaceAndOperStatusFlags._bitfield)
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

/// Connected WLAN details, when the machine is on Wi-Fi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiInfo {
    pub ssid: String,
    pub signal_percent: u32,
}

/// Current SSID and signal quality from the WLAN API.
///
/// Opens and closes a WLAN handle per call; callers cache it (the interface
/// list already refreshes every 15 s). Returns `None` when wired, disconnected,
/// or when the service is unavailable.
pub fn wifi() -> Option<WifiInfo> {
    let mut handle = HANDLE::default();
    let mut negotiated = 0u32;
    // SAFETY: standard WLAN client open; the out-parameters are valid.
    if unsafe { WlanOpenHandle(2, None, &mut negotiated, &mut handle) } != 0 {
        return None;
    }

    let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
    // SAFETY: `list` is a valid out-parameter that the API allocates.
    if unsafe { WlanEnumInterfaces(handle, None, &mut list) } != 0 || list.is_null() {
        // SAFETY: balances the successful open.
        unsafe {
            let _ = WlanCloseHandle(handle, None);
        }
        return None;
    }

    let mut best: Option<WifiInfo> = None;
    // SAFETY: on success the API returns a list with `dwNumberOfItems` entries
    // laid out contiguously from `InterfaceInfo`; every pointer is checked.
    unsafe {
        let count = (*list).dwNumberOfItems as usize;
        let infos = (*list).InterfaceInfo.as_ptr();
        for index in 0..count {
            let info = &*infos.add(index);
            if info.isState != wlan_interface_state_connected {
                continue;
            }

            let mut size = 0u32;
            let mut data: *mut c_void = std::ptr::null_mut();
            let status = WlanQueryInterface(
                handle,
                &info.InterfaceGuid,
                wlan_intf_opcode_current_connection,
                None,
                &mut size,
                &mut data,
                None,
            );
            if status != 0 || data.is_null() {
                continue;
            }

            let attributes = &*data.cast::<WLAN_CONNECTION_ATTRIBUTES>();
            let association = &attributes.wlanAssociationAttributes;
            let length = (association.dot11Ssid.uSSIDLength as usize).min(32);
            let ssid = String::from_utf8_lossy(&association.dot11Ssid.ucSSID[..length]).to_string();
            let signal = association.wlanSignalQuality.min(100);
            if !ssid.is_empty()
                && best
                    .as_ref()
                    .map(|current| signal > current.signal_percent)
                    .unwrap_or(true)
            {
                best = Some(WifiInfo {
                    ssid,
                    signal_percent: signal,
                });
            }
            WlanFreeMemory(data.cast());
        }
        WlanFreeMemory(list.cast());
        let _ = WlanCloseHandle(handle, None);
    }

    best
}
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
    fn filter_miniports_are_identified_by_the_ndis_flag() {
        assert!(is_filter_miniport(0b0000_0010));
        assert!(!is_filter_miniport(0b0000_0001));
        assert!(!is_filter_miniport(0));
    }

    #[test]
    fn wifi_query_does_not_panic() {
        if let Some(info) = wifi() {
            assert!(!info.ssid.is_empty());
            assert!(info.signal_percent <= 100);
        }
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
