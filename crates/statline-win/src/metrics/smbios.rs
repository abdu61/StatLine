//! SMBIOS memory device inventory (installed modules, size, speed, type).
//!
//! `GetSystemFirmwareTable('RSMB', …)` returns the raw SMBIOS structure table.
//! Only the `Type 17` ("Memory Device") records are read: present modules with a
//! non-zero size. This is static hardware, so the caller reads it once.

use windows::Win32::System::SystemInformation::{GetSystemFirmwareTable, RSMB};

/// One populated memory slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryModule {
    /// Slot label from the firmware, e.g. `DIMM 0` or `ChannelA-DIMM0`.
    pub slot: String,
    /// Bank/channel locator. Laptop firmware often repeats the device locator
    /// for both SODIMMs, so this is what tells them apart.
    pub bank: String,
    pub size_bytes: u64,
    /// Configured speed in MT/s.
    pub speed_mts: u32,
    /// Human name for the memory type (DDR4, LPDDR5, …).
    pub kind: String,
}

const SMBIOS_TYPE_MEMORY_DEVICE: u8 = 17;

/// Read the populated memory modules, in firmware order.
pub fn modules() -> Vec<MemoryModule> {
    let Some(table) = firmware_table() else {
        return Vec::new();
    };
    parse(&table)
}

fn firmware_table() -> Option<Vec<u8>> {
    // First call asks for the size only.
    // SAFETY: no buffers are passed; the call reports the required size.
    let needed = unsafe { GetSystemFirmwareTable(RSMB, 0, None) };
    if needed == 0 || needed as usize > 4 * 1024 * 1024 {
        return None;
    }

    let mut buffer = vec![0u8; needed as usize];
    // SAFETY: the buffer is exactly `needed` bytes, which is what the API wrote
    // on the sizing call; it reports the bytes actually written.
    let written = unsafe { GetSystemFirmwareTable(RSMB, 0, Some(&mut buffer)) };
    if written == 0 || written > needed {
        return None;
    }
    buffer.truncate(written as usize);
    Some(buffer)
}

/// Walk the structure table.
///
/// The `RSMB` buffer is `RawSMBIOSData`: one byte of calling method, major and
/// minor version, the DMI revision, then a little-endian DWORD table length,
/// then the structure table itself (8-byte header).
fn parse(table: &[u8]) -> Vec<MemoryModule> {
    const HEADER: usize = 8;
    if table.len() < HEADER {
        return Vec::new();
    }

    let declared = u32::from_le_bytes([table[4], table[5], table[6], table[7]]) as usize;
    let end = (HEADER + declared).min(table.len());
    let mut offset = HEADER;
    let mut modules = Vec::new();

    while offset + 4 <= end {
        let kind = table[offset];
        let length = table[offset + 1] as usize;
        if length < 4 || offset + length > end {
            break;
        }

        let formatted = &table[offset..offset + length];
        let strings = strings_after(&table[..end], offset + length);

        if kind == SMBIOS_TYPE_MEMORY_DEVICE {
            if let Some(module) = memory_device(formatted, &strings) {
                modules.push(module);
            }
        }
        if kind == 127 {
            break;
        }

        offset = next_structure(&table[..end], offset + length);
    }

    modules
}

/// Collect the NUL-separated strings of one structure until the double NUL.
fn strings_after(table: &[u8], start: usize) -> Vec<String> {
    let mut strings = Vec::new();
    let mut current = Vec::new();
    let mut index = start;
    let mut saw_any = false;

    while index < table.len() {
        let byte = table[index];
        if byte == 0 {
            if current.is_empty() {
                if saw_any || !strings.is_empty() {
                    break;
                }
                // A structure with no strings ends immediately.
                break;
            }
            strings.push(String::from_utf8_lossy(&current).trim().to_string());
            current.clear();
            saw_any = true;
            index += 1;
            continue;
        }
        current.push(byte);
        index += 1;
    }
    strings
}

/// Find the start of the next structure after `end` (past the double NUL).
fn next_structure(table: &[u8], mut index: usize) -> usize {
    while index + 1 < table.len() {
        if table[index] == 0 && table[index + 1] == 0 {
            return index + 2;
        }
        index += 1;
    }
    table.len()
}

fn memory_device(formatted: &[u8], strings: &[String]) -> Option<MemoryModule> {
    let field = |offset: usize| -> Option<u16> {
        (formatted.len() >= offset + 2)
            .then(|| u16::from_le_bytes([formatted[offset], formatted[offset + 1]]))
    };
    let byte_at = |offset: usize| -> Option<u8> { formatted.get(offset).copied() };

    // Type 17 offsets (SMBIOS 3.x): 0x0C size (bit 15 = KB, 0x7FFF = use the
    // extended size), 0x10 device locator, 0x12 memory type, 0x13 type detail
    // (a WORD, so speed lands at 0x15), 0x15 speed, 0x1C extended size,
    // 0x20 configured speed. Verified against a live DDR4-3200 SODIMM dump.
    let raw_size = field(0x0C)?;
    let size_bytes = if raw_size == 0x7FFF {
        u64::from(field(0x1C)?) * 1024 * 1024
    } else if raw_size & 0x8000 != 0 {
        u64::from(raw_size & 0x7FFF) * 1024
    } else {
        u64::from(raw_size) * 1024 * 1024
    };
    if size_bytes == 0 {
        return None;
    }

    let slot = string_at(strings, byte_at(0x10)?);
    if slot.is_empty() || slot == "Not Specified" {
        return None;
    }
    // 0x11 is the bank locator, another string index.
    let bank = byte_at(0x11)
        .map(|index| string_at(strings, index))
        .unwrap_or_default();

    let speed = field(0x20)
        .filter(|speed| *speed > 0)
        .or_else(|| field(0x15))
        .unwrap_or(0);
    let kind = memory_kind(byte_at(0x12).unwrap_or(0));

    Some(MemoryModule {
        slot,
        bank,
        size_bytes,
        speed_mts: u32::from(speed),
        kind,
    })
}

fn string_at(strings: &[String], index: u8) -> String {
    if index == 0 {
        return String::new();
    }
    strings.get(index as usize - 1).cloned().unwrap_or_default()
}

/// SMBIOS `Memory Type` values, with the DDR generation kept explicit.
fn memory_kind(memory_type: u8) -> String {
    let name = match memory_type {
        0x12 => "DDR",
        0x13 => "DDR2",
        0x18 => "DDR3",
        0x1A => "DDR4",
        0x22 => "DDR5",
        0x1B => "LPDDR",
        0x1C => "LPDDR2",
        0x1D => "LPDDR3",
        0x1E => "LPDDR4",
        0x23 => "LPDDR5",
        0x0F => "SDRAM",
        _ => "",
    };
    String::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Firmware coverage varies wildly: a physical laptop reports speed, a cloud
    /// VM often reports 0 or omits the field entirely. Assert the shape of what
    /// this machine offers rather than assuming it has the field at all — an
    /// earlier version of this test failed only on CI for exactly that reason.
    #[test]
    fn modules_report_plausible_identity() {
        for module in modules() {
            assert!(!module.slot.is_empty(), "slot label");

            // Speed is either absent (0) or a real MT/s figure. Nothing between.
            assert!(
                module.speed_mts == 0 || module.speed_mts >= 400,
                "speed is either 0 or MT/s, got {}",
                module.speed_mts
            );

            // An unknown generation is allowed; a wrong one is not.
            assert!(
                module.kind.is_empty()
                    || ["DDR3", "DDR4", "DDR5", "LPDDR3", "LPDDR4", "LPDDR5"]
                        .contains(&&*module.kind),
                "unexpected generation {:?}",
                module.kind
            );
        }
    }

    /// The parser's own logic, independent of what this machine's firmware says:
    /// a type 17 record with a configured speed yields it, one without yields 0.
    #[test]
    fn speed_is_read_from_the_record_that_has_it() {
        // Fields: 0x0C size = 8 GiB/1024, 0x10 locator string 1, 0x11 bank
        // string 2, 0x12 DDR4, 0x15 speed, 0x20 configured speed.
        let mut record = vec![0u8; 0x22];
        record[0] = SMBIOS_TYPE_MEMORY_DEVICE;
        record[1] = 0x22;
        record[0x0C] = 0x00; // low byte
        record[0x0D] = 0x40; // 0x4000 -> 16384 MiB with the high bit trick below
        record[0x10] = 1; // "DIMM 0"
        record[0x11] = 2; // "BANK 0"
        record[0x12] = 0x1A; // DDR4
        record[0x15] = 0x80; // 3200 MT/s, little-endian
        record[0x16] = 0x0C;
        record[0x20] = 0x80; // configured 3200 MT/s
        record[0x21] = 0x0C;

        let strings = vec![String::from("DIMM 0"), String::from("BANK 0")];
        let with_speed = memory_device(&record, &strings).expect("record parses");
        assert_eq!(with_speed.speed_mts, 3200);
        assert_eq!(with_speed.kind, "DDR4");
        assert_eq!(with_speed.bank, "BANK 0");

        // Clear both speed fields: the record must still parse, reporting 0.
        record[0x15] = 0;
        record[0x16] = 0;
        record[0x20] = 0;
        record[0x21] = 0;
        let without_speed = memory_device(&record, &strings).expect("record parses");
        assert_eq!(without_speed.speed_mts, 0);
    }

    #[test]
    fn module_list_is_plausible_when_present() {
        let modules = modules();
        let total: u64 = modules.iter().map(|module| module.size_bytes).sum();
        for module in &modules {
            assert!(!module.slot.is_empty());
            assert!(module.size_bytes >= 1024 * 1024);
        }
        if !modules.is_empty() {
            assert!(total > 0);
        }
    }

    #[test]
    fn next_structure_skips_the_double_nul() {
        let table = [0u8, 0, b'A', 0, 0, 9];
        assert_eq!(next_structure(&table, 2), 5);
    }

    #[test]
    fn strings_are_read_until_the_double_nul() {
        let table = b"A\0B\0\0rest";
        let strings = strings_after(table, 0);
        assert_eq!(strings, vec![String::from("A"), String::from("B")]);
    }
}
