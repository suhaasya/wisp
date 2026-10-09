//! `settings.toml` schema (versioned).

use serde::{Deserialize, Serialize};

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub appearance: AppearanceSettings,
    #[serde(default)]
    pub keybindings: KeybindingsSettings,
    #[serde(default)]
    pub grid: GridSettings,
    #[serde(default)]
    pub results: ResultsSettings,
    #[serde(default)]
    pub telemetry: TelemetrySettings,
}

fn default_version() -> u32 {
    SETTINGS_VERSION
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            appearance: AppearanceSettings::default(),
            keybindings: KeybindingsSettings::default(),
            grid: GridSettings::default(),
            results: ResultsSettings::default(),
            telemetry: TelemetrySettings::default(),
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeModeSetting {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DensitySetting {
    #[default]
    Compact,
    Comfortable,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFontSetting {
    #[default]
    System,
    IbmPlexSans,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonoFontSetting {
    #[default]
    System,
    IbmPlexMono,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct KeybindingsSettings {
    #[serde(default)]
    pub bindings: Vec<KeybindingEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeybindingEntry {
    pub action: String,
    pub keystroke: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridSettings {
    #[serde(default = "default_grid_page_size")]
    pub page_size: u32,
    #[serde(default = "default_row_detail_width")]
    pub row_detail_width: f32,
}

fn default_row_detail_width() -> f32 {
    300.0
}

fn default_grid_page_size() -> u32 {
    200
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            page_size: default_grid_page_size(),
            row_detail_width: default_row_detail_width(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultsSettings {
    #[serde(default = "default_result_cap_mb")]
    pub cap_mb: u32,
}

fn default_result_cap_mb() -> u32 {
    32
}

impl Default for ResultsSettings {
    fn default() -> Self {
        Self {
            cap_mb: default_result_cap_mb(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TelemetrySettings {
    #[serde(default)]
    pub opt_in: bool,
}
