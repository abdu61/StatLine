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
    fn percentages_round_and_clamp() {
        assert_eq!(format_percent(0.4), "0%");
        assert_eq!(format_percent(23.4), "23%");
        assert_eq!(format_percent(99.6), "100%");
        assert_eq!(format_percent(150.0), "100%");
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
