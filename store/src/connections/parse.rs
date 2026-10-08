//! Parse and validate `connections.toml`.

use thiserror::Error;

use super::{
    migrate::migrate,
    schema::{ConnectionsFile, CONNECTIONS_VERSION},
};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConnectionsParseError {
    #[error("unsupported connections version {0}")]
    UnsupportedVersion(u32),
    #[error("invalid connections.toml at line {line}: {message}")]
    Syntax { line: usize, message: String },
    #[error("invalid connections.toml: {0}")]
    Toml(String),
}

impl ConnectionsParseError {
    pub fn line(&self) -> Option<usize> {
        match self {
            Self::Syntax { line, .. } => Some(*line),
            _ => None,
        }
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::Syntax { line, message } => format!(
                "Saved connections could not be read (line {line}: {message}). A backup was created; fix the file or delete it to start fresh."
            ),
            Self::Toml(message) => format!(
                "Saved connections could not be read ({message}). A backup was created; fix the file or delete it to start fresh."
            ),
            Self::UnsupportedVersion(v) => format!(
                "Saved connections use unsupported version {v}. A backup was created."
            ),
        }
    }

    fn from_toml_de(text: &str, err: toml::de::Error) -> Self {
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

pub fn parse_connections_toml(text: &str) -> Result<ConnectionsFile, ConnectionsParseError> {
    let file: ConnectionsFile =
        toml::from_str(text).map_err(|e| ConnectionsParseError::from_toml_de(text, e))?;
    if file.version > CONNECTIONS_VERSION {
        return Err(ConnectionsParseError::UnsupportedVersion(file.version));
    }
    Ok(migrate(file))
}
