//! Schema migrations for `settings.toml`.

use super::schema::{Settings, SETTINGS_VERSION};
use crate::settings::parse::SettingsParseError;

/// Legacy v0 on-disk shape (no `version`, flat appearance keys).
#[derive(Debug, serde::Deserialize)]
pub struct SettingsV0 {
    #[serde(default)]
    pub theme_mode: super::schema::ThemeModeSetting,
    #[serde(default)]
    pub density: super::schema::DensitySetting,
    #[serde(default)]
    pub ui_font: super::schema::UiFontSetting,
    #[serde(default)]
    pub mono_font: super::schema::MonoFontSetting,
}

pub fn migrate_document(text: &str, raw: toml::Value) -> Result<Settings, SettingsParseError> {
    let version = raw
        .get("version")
        .and_then(|v| v.as_integer())
        .unwrap_or(0) as u32;

    match version {
        SETTINGS_VERSION => raw
            .try_into()
            .map_err(|e| SettingsParseError::from_toml_de(text, e)),
        0 => migrate_v0(text, raw),
        other => Err(SettingsParseError::UnsupportedVersion(other)),
    }
}

fn migrate_v0(text: &str, raw: toml::Value) -> Result<Settings, SettingsParseError> {
    if raw.get("appearance").is_some() {
        let mut settings: Settings = raw
            .try_into()
            .map_err(|e| SettingsParseError::from_toml_de(text, e))?;
        settings.version = SETTINGS_VERSION;
        return Ok(settings);
    }
    let legacy: SettingsV0 = raw
        .try_into()
        .map_err(|e| SettingsParseError::from_toml_de(text, e))?;
    Ok(Settings {
        version: SETTINGS_VERSION,
        appearance: super::schema::AppearanceSettings {
            theme_mode: legacy.theme_mode,
            density: legacy.density,
            ui_font: legacy.ui_font,
            mono_font: legacy.mono_font,
        },
        ..Settings::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::schema::{DensitySetting, ThemeModeSetting};

    #[test]
    fn migrates_v0_flat_sample() {
        let text = r#"
theme_mode = "dark"
density = "comfortable"
"#;
        let raw: toml::Value = toml::from_str(text).unwrap();
        let settings = migrate_v0(text, raw).unwrap();
        assert_eq!(settings.version, SETTINGS_VERSION);
        assert_eq!(settings.appearance.theme_mode, ThemeModeSetting::Dark);
        assert_eq!(settings.appearance.density, DensitySetting::Comfortable);
    }
}
