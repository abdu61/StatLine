//! Settings schema, persistence and migration.
//!
//! Design rules:
//! - Every on-disk change bumps `schema_version` and gets a migration path.
//! - Writes are atomic (temp file + rename) so a crash never truncates settings.
//! - A corrupt file is quarantined, not silently overwritten with defaults.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Current on-disk schema version.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid settings JSON: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Root settings document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u32,
    pub general: General,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            general: General::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    /// Sample interval in milliseconds.
    pub update_interval_ms: u32,
    /// Start with Windows.
    pub run_at_startup: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            update_interval_ms: 1000,
            run_at_startup: false,
        }
    }
}

impl Settings {
    /// Load settings, falling back to defaults when the file is absent.
    ///
    /// A malformed file is moved aside to `<name>.corrupt-<n>` so the user can
    /// recover it, and defaults are returned. I/O errors other than "not found"
    /// are reported.
    pub fn load_or_default(path: &Path) -> Result<Self, ConfigError> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(ConfigError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };

        match serde_json::from_slice::<Self>(&bytes) {
            Ok(settings) => Ok(settings),
            Err(_) => {
                let _ = std::fs::rename(path, path.with_extension("json.corrupt"));
                Ok(Self::default())
            }
        }
    }

    /// Write settings atomically: serialize, write to a sibling temp file, then
    /// rename over the target.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let json = serde_json::to_vec_pretty(self)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &json).map_err(|source| ConfigError::Write {
            path: tmp.clone(),
            source,
        })?;
        std::fs::rename(&tmp, path).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip() {
        let json = serde_json::to_string(&Settings::default()).expect("serialize");
        let back: Settings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, Settings::default());
    }

    #[test]
    fn missing_file_yields_defaults() {
        let dir = std::env::temp_dir().join("statline-config-test-missing");
        let _ = std::fs::remove_dir_all(&dir);
        let settings = Settings::load_or_default(&dir.join("settings.json")).expect("load");
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn save_and_reload() {
        let dir = std::env::temp_dir().join("statline-config-test-roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("settings.json");

        let mut settings = Settings::default();
        settings.general.update_interval_ms = 2000;
        settings.save(&path).expect("save");

        let loaded = Settings::load_or_default(&path).expect("load");
        assert_eq!(loaded.general.update_interval_ms, 2000);
    }

    #[test]
    fn corrupt_file_is_quarantined() {
        let dir = std::env::temp_dir().join("statline-config-test-corrupt");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join("settings.json");
        std::fs::write(&path, b"{ this is not json").expect("write");

        let loaded = Settings::load_or_default(&path).expect("load");
        assert_eq!(loaded, Settings::default());
        assert!(!path.exists(), "corrupt file should be moved aside");
    }
}
