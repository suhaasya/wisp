//! TOML parse errors with line numbers.

use thiserror::Error;

use super::migrate::migrate_document;
use super::schema::Settings;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SettingsParseError {
    #[error("unsupported settings version {0}")]
    UnsupportedVersion(u32),
    #[error("invalid settings.toml at line {line}: {message}")]
    Syntax { line: usize, message: String },
    #[error("invalid settings.toml: {0}")]
    Toml(String),
}

impl SettingsParseError {
    pub fn line(&self) -> Option<usize> {
        match self {
            Self::Syntax { line, .. } => Some(*line),
            _ => None,
        }
    }

    pub fn from_toml_de(text: &str, err: toml::de::Error) -> Self {
        if let Some(span) = err.span() {
            let end = span.start.min(text.len());
            let line = text[..end].lines().count().max(1);
            Self::Syntax {
                line,
                message: err.message().to_string(),
            }
        } else {
            Self::Toml(err.message().to_string())
        }
    }
}

pub fn parse_settings_toml(text: &str) -> Result<Settings, SettingsParseError> {
    let raw: toml::Value = toml::from_str(text).map_err(|e| {
        if let Some(span) = e.span() {
            let end = span.start.min(text.len());
            let line = text[..end].lines().count().max(1);
            SettingsParseError::Syntax {
                line,
                message: e.message().to_string(),
            }
        } else {
            SettingsParseError::Toml(e.message().to_string())
        }
    })?;
    migrate_document(text, raw)
}
