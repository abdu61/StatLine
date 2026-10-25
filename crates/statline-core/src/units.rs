//! Human-readable formatting for every number the UI shows.

/// Which base to use for byte quantities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnitSystem {
    /// 1024-based: B, KiB, MiB, GiB, TiB.
    #[default]
    Binary,
    /// 1000-based: B, KB, MB, GB, TB.
    Decimal,
}

impl UnitSystem {
    fn base(self) -> f64 {
        match self {
            Self::Binary => 1024.0,
            Self::Decimal => 1000.0,
        }
    }

    fn suffixes(self) -> [&'static str; 5] {
        match self {
            Self::Binary => ["B", "KiB", "MiB", "GiB", "TiB"],
            Self::Decimal => ["B", "KB", "MB", "GB", "TB"],
        }
    }
}

/// `1536` → `"1.5 KiB"`.
pub fn format_bytes(bytes: u64, units: UnitSystem) -> String {
    format_scaled(bytes as f64, units, "")
}

/// `1_310_720.0` → `"1.2 MiB/s"`.
pub fn format_rate(bytes_per_sec: f64, units: UnitSystem) -> String {
    format_scaled(bytes_per_sec, units, "/s")
}

fn format_scaled(value: f64, units: UnitSystem, suffix: &str) -> String {
    if !value.is_finite() || value <= 0.0 {
        return format!("0 B{suffix}");
    }

    let base = units.base();
    let suffixes = units.suffixes();
    let mut scaled = value;
    let mut index = 0;
    while scaled >= base && index + 1 < suffixes.len() {
        scaled /= base;
        index += 1;
    }

    let decimals = if scaled < 10.0 && index > 0 { 1 } else { 0 };
    format!("{scaled:.decimals$} {}{suffix}", suffixes[index])
}

/// `23.4` → `"23%"`. Sub-one values round to zero rather than inventing "<1%".
pub fn format_percent(percent: f32) -> String {
    let rounded = percent.clamp(0.0, 100.0).round() as i32;
    format!("{rounded}%")
}

/// `4.24` → `"4.2%"`, `23.4` → `"23%"`.
///
/// Process rows need the extra digit: the difference between an idle app and
/// one using 0.4 % of the machine is exactly what the table is for.
pub fn format_percent_precise(percent: f32) -> String {
    let clamped = percent.clamp(0.0, 100.0);
    if clamped < 10.0 {
        format!("{clamped:.1}%")
    } else {
        format!("{clamped:.0}%")
    }
}

/// `62.4` → `"62 °C"`.
pub fn format_temperature(celsius: f32) -> String {
    format!("{:.0} °C", celsius)
}

/// `90_061` → `"1d 1h"`, `14_400` → `"4h 0m"`, `45` → `"45s"`.
pub fn format_duration(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;

    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{seconds}s")
    }
}

/// `2400` → `"2.4 GHz"`, `800` → `"800 MHz"`.
pub fn format_frequency(mhz: u32) -> String {
    if mhz >= 1000 {
        format!("{:.1} GHz", mhz as f64 / 1000.0)
    } else {
        format!("{mhz} MHz")
    }
}

/// Very compact rate for the taskbar strip: 1_240_000 becomes "1.2M".
///
/// The strip has no room for a unit per value, so the suffix replaces both the
/// unit and the "/s"; the segment label says what the numbers mean.
pub fn format_rate_compact(bytes_per_sec: f64, units: UnitSystem) -> String {
    if !bytes_per_sec.is_finite() || bytes_per_sec <= 0.0 {
        return String::from("0");
    }

    let base = units.base();
    let suffixes = ["B", "K", "M", "G", "T"];
    let mut scaled = bytes_per_sec;
    let mut index = 0;
    while scaled >= base && index + 1 < suffixes.len() {
        scaled /= base;
        index += 1;
    }

    if index == 0 {
        format!("{scaled:.0}B")
    } else if scaled < 10.0 {
        format!("{scaled:.1}{}", suffixes[index])
    } else {
        format!("{scaled:.0}{}", suffixes[index])
    }
}

/// Short rate for the strip: three significant digits, adaptive unit.
///
/// Examples: `1.24 KiB/s`, `82.0 KiB/s`, `999 KiB/s`. Three digits keep the
/// slot width stable while a value changes; the unit changes only when the
/// magnitude does. A value that would round to 1000 is promoted to the next
/// unit, so the number never grows a fourth digit.
pub fn format_rate_short(bytes_per_sec: f64, units: UnitSystem) -> String {
    if !bytes_per_sec.is_finite() || bytes_per_sec <= 0.0 {
        return String::from("0 B/s");
    }

    let base = units.base();
    let suffixes = units.suffixes();
    let mut scaled = bytes_per_sec;
    let mut index = 0;
    while scaled >= base && index + 1 < suffixes.len() {
        scaled /= base;
        index += 1;
    }
    if scaled >= 999.5 && index + 1 < suffixes.len() {
        scaled /= base;
        index += 1;
    }

    let decimals = if index == 0 || scaled >= 99.95 {
        0
    } else if scaled >= 9.995 {
        1
    } else {
        2
    };
    format!("{scaled:.decimals$} {}/s", suffixes[index])
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_use_the_expected_suffix() {
        assert_eq!(format_bytes(0, UnitSystem::Binary), "0 B");
        assert_eq!(format_bytes(512, UnitSystem::Binary), "512 B");
        assert_eq!(format_bytes(1536, UnitSystem::Binary), "1.5 KiB");
        assert_eq!(format_bytes(1024 * 1024, UnitSystem::Binary), "1.0 MiB");
        assert_eq!(
            format_bytes(16 * 1024 * 1024 * 1024, UnitSystem::Binary),
            "16 GiB"
        );
    }

    #[test]
    fn decimal_units_differ_from_binary() {
        assert_eq!(format_bytes(1_500_000, UnitSystem::Decimal), "1.5 MB");
        assert_eq!(format_bytes(1_500_000, UnitSystem::Binary), "1.4 MiB");
    }

    #[test]
    fn rates_append_per_second() {
        assert_eq!(format_rate(0.0, UnitSystem::Binary), "0 B/s");
        assert_eq!(format_rate(1_310_720.0, UnitSystem::Binary), "1.2 MiB/s");
        assert_eq!(format_rate(412.0, UnitSystem::Decimal), "412 B/s");
    }

    #[test]
    fn compact_rates_drop_the_unit_suffix() {
        assert_eq!(format_rate_compact(0.0, UnitSystem::Binary), "0");
        assert_eq!(format_rate_compact(512.0, UnitSystem::Binary), "512B");
        assert_eq!(format_rate_compact(1_500.0, UnitSystem::Binary), "1.5K");
        assert_eq!(format_rate_compact(84_000.0, UnitSystem::Binary), "82K");
        assert_eq!(format_rate_compact(1_240_000.0, UnitSystem::Binary), "1.2M");
        assert_eq!(
            format_rate_compact(1_240_000.0, UnitSystem::Decimal),
            "1.2M"
        );
    }

    #[test]
    fn percentages_round_and_clamp() {
        assert_eq!(format_percent(0.4), "0%");
        assert_eq!(format_percent(23.4), "23%");
        assert_eq!(format_percent(99.6), "100%");
        assert_eq!(format_percent(150.0), "100%");
    }

    #[test]
    fn precise_percentages_keep_a_digit_below_ten() {
        assert_eq!(format_percent_precise(0.4), "0.4%");
        assert_eq!(format_percent_precise(4.24), "4.2%");
        assert_eq!(format_percent_precise(9.96), "10.0%");
        assert_eq!(format_percent_precise(23.4), "23%");
        assert_eq!(format_percent_precise(150.0), "100%");
    }

    #[test]
    fn durations_pick_the_largest_unit() {
        assert_eq!(format_duration(45), "45s");
        assert_eq!(format_duration(600), "10m");
        assert_eq!(format_duration(14_400), "4h 0m");
        assert_eq!(format_duration(90_061), "1d 1h");
    }

    #[test]
    fn frequency_switches_to_ghz() {
        assert_eq!(format_frequency(800), "800 MHz");
        assert_eq!(format_frequency(2400), "2.4 GHz");
    }

    #[test]
    fn temperature_has_one_suffix() {
        assert_eq!(format_temperature(62.4), "62 °C");
    }
}
