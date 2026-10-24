//! Minimal file logger.
//!
//! Deliberately small: one file, UTC timestamps, `log` facade so the rest of
//! the codebase stays backend-agnostic. Rotation arrives with M4.

use log::{LevelFilter, Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

struct FileLogger {
    file: Mutex<File>,
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} {:<5} {}: {}\r\n",
            format_timestamp(SystemTime::now()),
            record.level(),
            record.target(),
            record.args()
        );
        if let Ok(mut file) = self.file.lock() {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

/// Install the file logger. Safe to call twice; the second call is a no-op.
pub fn init(log_dir: &Path, level: LevelFilter) -> std::io::Result<()> {
    std::fs::create_dir_all(log_dir)?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("statline.log"))?;

    let logger = FileLogger {
        file: Mutex::new(file),
    };
    // SAFETY: `set_boxed_logger` may only be called once per process. We ignore
    // the error if a logger is already installed, which keeps this idempotent.
    match log::set_boxed_logger(Box::new(logger)) {
        Ok(()) => log::set_max_level(level),
        Err(_) => log::set_max_level(level),
    }
    Ok(())
}

/// Render a `SystemTime` as `YYYY-MM-DD HH:MM:SS.mmm` in UTC.
///
/// Implemented locally to avoid pulling a date/time crate into the binary.
fn format_timestamp(time: SystemTime) -> String {
    let since_epoch = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_secs = since_epoch.as_secs() as i64;
    let millis = since_epoch.subsec_millis();

    let days = total_secs.div_euclid(86_400);
    let secs_of_day = total_secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Howard Hinnant's days-to-civil-date algorithm (proleptic Gregorian).
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn formats_known_instants() {
        // 2024-02-29T12:34:56.789Z — leap day sanity check.
        let time = UNIX_EPOCH + Duration::from_millis(1_709_210_096_789);
        assert_eq!(format_timestamp(time), "2024-02-29 12:34:56.789Z");
    }

    #[test]
    fn formats_epoch() {
        assert_eq!(format_timestamp(UNIX_EPOCH), "1970-01-01 00:00:00.000Z");
    }
}
