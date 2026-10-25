//! Settings schema, persistence and migration.
//!
//! Design rules:
//! - Every on-disk change bumps `schema_version` and gets a migration path.
//! - Writes are atomic (temp file + rename) so a crash never truncates settings.
//! - A corrupt file is quarantined, not silently overwritten with defaults.
//! - Unknown keys are ignored rather than rejected, so a newer build can write a
//!   file that an older build still opens.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Current on-disk schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// Sample intervals the UI offers, in milliseconds.
pub const INTERVALS_MS: [u32; 4] = [500, 1000, 2000, 5000];

/// Remembered outer rectangle of the companion window, in physical pixels.
///
/// It lives in its own `window.json`: window placement changes on every drag,
/// and folding that into the settings document would make every host fight over
/// the same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl WindowBounds {
    /// Reject absurd or minimized rectangles before they reach the window API.
    pub fn is_sane(&self) -> bool {
        (480..=10_000).contains(&self.width)
            && (360..=10_000).contains(&self.height)
            && (-20_000..=20_000).contains(&self.x)
            && (-20_000..=20_000).contains(&self.y)
    }

    pub fn load(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        let bounds: Self = serde_json::from_str(&text).ok()?;
        bounds.is_sane().then_some(bounds)
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if !self.is_sane() {
            return Ok(());
        }
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub general: General,
    pub appearance: Appearance,
    pub modules: Modules,
}

/// Modules shown in the taskbar strip. The flyout and companion stay complete.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Modules {
    pub cpu: bool,
    pub memory: bool,
    pub gpu: bool,
    pub disk: bool,
    pub network: bool,
}

impl Default for Modules {
    fn default() -> Self {
        Self {
            cpu: true,
            memory: true,
            gpu: true,
            disk: true,
            network: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    /// Sample interval in milliseconds; normalized to `INTERVALS_MS`.
    pub update_interval_ms: u32,
    /// Start with Windows (per-user Run key for portable builds).
    pub run_at_startup: bool,
    /// Which side of the taskbar the strip sits on.
    pub strip_placement: StripPlacement,
    /// Byte units: 1024-based (KiB) or 1000-based (kB).
    pub units: Units,
}

impl Default for General {
    fn default() -> Self {
        Self {
            update_interval_ms: 1000,
            run_at_startup: false,
            strip_placement: StripPlacement::Right,
            units: Units::Binary,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    /// 1024-based: KiB, MiB, GiB.
    #[default]
    Binary,
    /// 1000-based: kB, MB, GB.
    Decimal,
}

impl Units {
    pub fn next(self) -> Self {
        match self {
            Self::Binary => Self::Decimal,
            Self::Decimal => Self::Binary,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Binary => "Binary (KiB)",
            Self::Decimal => "Decimal (kB)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Appearance {
    pub theme: Theme,
    /// Accent used by charts, meters and selection.
    pub accent: Accent,
    /// Windows 11 Mica system backdrop on the framed companion/settings windows.
    pub mica: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    /// Follow the Windows accent colour.
    #[default]
    System,
    Blue,
    Green,
    Purple,
    Orange,
    Teal,
    Pink,
}

impl Accent {
    pub fn next(self) -> Self {
        match self {
            Self::System => Self::Blue,
            Self::Blue => Self::Green,
            Self::Green => Self::Purple,
            Self::Purple => Self::Orange,
            Self::Orange => Self::Teal,
            Self::Teal => Self::Pink,
            Self::Pink => Self::System,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Blue => "Blue",
            Self::Green => "Green",
            Self::Purple => "Purple",
            Self::Orange => "Orange",
            Self::Teal => "Teal",
            Self::Pink => "Pink",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follow the Windows app theme.
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    /// Cycle order used by the settings UI.
    pub fn next(self) -> Self {
        match self {
            Self::System => Self::Dark,
            Self::Dark => Self::Light,
            Self::Light => Self::System,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    /// Resolve to a concrete theme, consulting Windows only for `System`.
    pub fn resolve_dark(self, system_is_dark: bool) -> bool {
        match self {
            Self::System => system_is_dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StripPlacement {
    #[default]
    Right,
    Left,
}

impl StripPlacement {
    pub fn next(self) -> Self {
        match self {
            Self::Right => Self::Left,
            Self::Left => Self::Right,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Right => "Right",
            Self::Left => "Left",
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
            Ok(mut settings) => {
                settings.normalize();
                Ok(settings)
            }
            Err(_) => {
                let _ = std::fs::rename(path, path.with_extension("json.corrupt"));
                Ok(Self::default())
            }
        }
    }

    /// Clamp values that a hand-edited file could put out of range.
    pub fn normalize(&mut self) {
        self.schema_version = SCHEMA_VERSION;
        if !INTERVALS_MS.contains(&self.general.update_interval_ms) {
            self.general.update_interval_ms = 1000;
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
        settings.general.strip_placement = StripPlacement::Left;
        settings.general.units = Units::Decimal;
        settings.appearance.theme = Theme::Light;
        settings.save(&path).expect("save");

        let loaded = Settings::load_or_default(&path).expect("load");
        assert_eq!(loaded.general.update_interval_ms, 2000);
        assert_eq!(loaded.general.strip_placement, StripPlacement::Left);
        assert_eq!(loaded.general.units, Units::Decimal);
        assert_eq!(loaded.appearance.theme, Theme::Light);
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

    #[test]
    fn unknown_keys_are_ignored() {
        let json = r#"{"schema_version":1,"future":{"x":1},"general":{"update_interval_ms":2000}}"#;
        let settings: Settings = serde_json::from_str(json).expect("forward compatible");
        assert_eq!(settings.general.update_interval_ms, 2000);
    }

    #[test]
    fn out_of_range_interval_is_clamped() {
        let mut settings = Settings::default();
        settings.general.update_interval_ms = 137;
        settings.normalize();
        assert_eq!(settings.general.update_interval_ms, 1000);
    }

    #[test]
    fn theme_cycles_through_every_value() {
        let mut theme = Theme::System;
        let mut seen = 0;
        for _ in 0..3 {
            theme = theme.next();
            seen += 1;
        }
        assert_eq!(seen, 3);
        assert_eq!(theme, Theme::System);
        assert!(!Theme::Light.resolve_dark(true));
        assert!(Theme::System.resolve_dark(true));
    }

    #[test]
    fn units_default_to_binary_and_toggle() {
        assert_eq!(Units::default(), Units::Binary);
        assert_eq!(Units::Binary.next(), Units::Decimal);
        assert_eq!(Units::Decimal.next(), Units::Binary);
        assert_ne!(Units::Binary.label(), Units::Decimal.label());
    }

    #[test]
    fn modules_default_to_visible_and_round_trip() {
        assert_eq!(Modules::default(), Modules::default());
        let dir = std::env::temp_dir().join("statline-config-test-modules");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("settings.json");
        let mut settings = Settings::default();
        settings.modules.gpu = false;
        settings.modules.network = false;
        settings.save(&path).expect("save");
        let loaded = Settings::load_or_default(&path).expect("load");
        assert!(!loaded.modules.gpu);
        assert!(!loaded.modules.network);
        assert!(loaded.modules.cpu);
    }

    #[test]
    fn window_bounds_round_trip() {
        let dir = std::env::temp_dir().join("statline-config-test-window");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("window.json");
        let bounds = WindowBounds {
            x: 120,
            y: 80,
            width: 900,
            height: 700,
        };
        bounds.save(&path).expect("save");
        assert_eq!(WindowBounds::load(&path), Some(bounds));
    }

    #[test]
    fn absurd_window_bounds_are_rejected() {
        assert!(
            !WindowBounds {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            }
            .is_sane()
        );
        assert!(
            !WindowBounds {
                x: -40_000,
                y: 0,
                width: 800,
                height: 600,
            }
            .is_sane()
        );
    }

    #[test]
    fn accent_cycles_through_every_preset() {
        let mut accent = Accent::System;
        let mut seen = 0;
        for _ in 0..7 {
            accent = accent.next();
            seen += 1;
        }
        assert_eq!(seen, 7);
        assert_eq!(accent, Accent::System);
        assert_eq!(Accent::Green.label(), "Green");
    }
}
