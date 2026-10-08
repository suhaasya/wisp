//! Driver-level errors (mapped to [`wisp_core::WispError`] at the session layer).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("{message}")]
    User {
        message: String,
        detail: String,
    },
    #[error("not connected")]
    NotConnected,
    #[error("query cancelled")]
    Cancelled,
    #[error("invalid page request")]
    InvalidPage,
}

impl DriverError {
    pub fn user(message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::User {
            message: message.into(),
            detail: detail.into(),
        }
    }
}
