//! User settings persisted on disk (appearance wired in LUM-006; full UI in LUM-008).

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

const FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppearanceSettings {
    #[serde(default)]
    pub theme_mode: ThemeModeSetting,
    #[serde(default)]
    pub density: DensitySetting,
    #[serde(default)]
    pub ui_font: UiFontSetting,
    #[serde(default)]
    pub mono_font: MonoFontSetting,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeModeSetting::System,
            density: DensitySetting::Compact,
            ui_font: UiFontSetting::System,
            mono_font: MonoFontSetting::System,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeModeSetting {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DensitySetting {
    #[default]
    Compact,
    Comfortable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFontSetting {
    #[default]
    System,
    IbmPlexSans,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonoFontSetting {
    #[default]
    System,
    IbmPlexMono,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub appearance: AppearanceSettings,
}

impl Settings {
    pub fn load() -> Self {
        let path = settings_path();
        if !path.is_file() {
            return Self::default();
        }
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(self)?)
    }
}

pub fn settings_path() -> PathBuf {
    if let Some(dirs) = directories::ProjectDirs::from("", "", "wisp") {
        dirs.config_dir().join(FILE_NAME)
    } else {
        PathBuf::from(".wisp/settings.json")
    }
}
