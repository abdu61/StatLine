//! Filesystem locations used by StatLine.
//!
//! Settings live in `%APPDATA%` (roaming, user-editable); logs live in
//! `%LOCALAPPDATA%` (machine-local, disposable).

use std::path::PathBuf;

const APP_DIR: &str = "StatLine";

fn env_path(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(fallback))
}

/// Roaming application data directory (settings).
pub fn data_dir() -> PathBuf {
    env_path("APPDATA", "StatLine-roaming").join(APP_DIR)
}

/// Local application data directory (logs, caches).
pub fn local_dir() -> PathBuf {
    env_path("LOCALAPPDATA", "StatLine-local").join(APP_DIR)
}

/// Directory containing rotating log files.
pub fn log_dir() -> PathBuf {
    local_dir().join("logs")
}

/// Path to the settings document.
pub fn settings_file() -> PathBuf {
    data_dir().join("settings.json")
}

/// Path to the remembered companion window rectangle.
pub fn window_state_file() -> PathBuf {
    data_dir().join("window.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_end_with_app_name() {
        assert!(data_dir().ends_with(APP_DIR));
        assert!(log_dir().ends_with("logs"));
        assert!(settings_file().ends_with("settings.json"));
        assert!(window_state_file().ends_with("window.json"));
    }
}
