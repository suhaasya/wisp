//! Versioned `settings.toml` load/save, migration, and file watching.

mod io;
mod migrate;
mod parse;
mod schema;
mod watch;

pub use parse::{parse_settings_toml, SettingsParseError};
pub use schema::{
    AppearanceSettings, DensitySetting, GridSettings, KeybindingEntry, KeybindingsSettings,
    MonoFontSetting, ResultsSettings, Settings, TelemetrySettings, ThemeModeSetting, UiFontSetting,
    SETTINGS_VERSION,
};
pub use watch::{spawn_settings_watcher, SettingsWatchEvent};

use std::path::{Path, PathBuf};

use crate::paths::WispPaths;

/// In-memory settings with durable TOML backing.
#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
    current: Settings,
}

impl SettingsStore {
    pub fn load() -> Self {
        Self::load_from_paths(WispPaths::resolve())
    }

    pub fn load_from_paths(paths: WispPaths) -> Self {
        let path = paths.settings_toml();
        let current = match std::fs::read_to_string(&path) {
            Ok(text) => parse_settings_toml(&text).unwrap_or_else(|_| Settings::default()),
            Err(_) if !path.is_file() => Settings::default(),
            Err(_) => Settings::default(),
        };
        Self { path, current }
    }

    pub fn load_or_default(path: &Path) -> Self {
        let current = if !path.is_file() {
            Settings::default()
        } else {
            match std::fs::read_to_string(path) {
                Ok(text) => parse_settings_toml(&text).unwrap_or_default(),
                Err(_) => Settings::default(),
            }
        };
        Self {
            path: path.to_path_buf(),
            current,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn get(&self) -> &Settings {
        &self.current
    }

    pub fn replace(&mut self, settings: Settings) {
        self.current = settings;
    }

    pub fn set(&mut self, settings: Settings) {
        self.current = settings;
    }

    pub fn save(&self) -> std::io::Result<()> {
        io::write_settings_atomic(&self.path, &self.current)
    }

    /// Reload from disk; on parse failure keeps the in-memory snapshot.
    pub fn try_reload(&mut self) -> Result<Settings, SettingsParseError> {
        if !self.path.is_file() {
            self.current = Settings::default();
            return Ok(self.current.clone());
        }
        let text = std::fs::read_to_string(&self.path).map_err(|e| SettingsParseError::Toml(e.to_string()))?;
        match parse_settings_toml(&text) {
            Ok(settings) => {
                self.current = settings.clone();
                Ok(settings)
            }
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::settings::schema::{DensitySetting, ThemeModeSetting};

    #[test]
    fn round_trip_defaults_write_read_equal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("settings.toml");
        let store = SettingsStore {
            path: path.clone(),
            current: Settings::default(),
        };
        store.save().expect("save");
        let loaded = SettingsStore::load_or_default(&path);
        assert_eq!(loaded.get(), store.get());
    }

    #[test]
    fn missing_file_uses_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("missing.toml");
        let store = SettingsStore::load_or_default(&path);
        assert_eq!(store.get(), &Settings::default());
    }

    #[test]
    fn invalid_toml_keeps_last_good_on_reload() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("settings.toml");
        let mut store = SettingsStore {
            path: path.clone(),
            current: Settings {
                appearance: AppearanceSettings {
                    density: DensitySetting::Comfortable,
                    ..AppearanceSettings::default()
                },
                ..Settings::default()
            },
        };
        store.save().expect("save");
        fs::write(&path, "version = 1\nappearance = {").expect("write bad");
        let err = store.try_reload().expect_err("parse should fail");
        assert!(err.line().is_some() || matches!(err, SettingsParseError::Toml(_)));
        assert_eq!(
            store.get().appearance.density,
            DensitySetting::Comfortable
        );
    }

    #[test]
    fn migrate_v0_sample_file() {
        let text = include_str!("../../tests/fixtures/settings_v0.toml");
        let settings = parse_settings_toml(text).expect("migrate v0");
        assert_eq!(settings.version, SETTINGS_VERSION);
        assert_eq!(settings.appearance.theme_mode, ThemeModeSetting::Dark);
    }
}
