//! User-visible and diagnostic errors shared across Wisp crates.

use thiserror::Error;

/// Application error with a short user-facing line and optional diagnostic detail.
#[derive(Debug, Clone, Error)]
#[error("{message}")]
pub struct WispError {
    message: String,
    detail: String,
    kind: WispErrorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WispErrorKind {
    Cancelled,
    Internal,
}

impl WispError {
    pub fn new(
        message: impl Into<String>,
        detail: impl Into<String>,
        kind: WispErrorKind,
    ) -> Self {
        Self {
            message: message.into(),
            detail: detail.into(),
            kind,
        }
    }

    pub fn cancelled() -> Self {
        Self::new(
            "The operation was cancelled.",
            "cancellation token fired",
            WispErrorKind::Cancelled,
        )
    }

    pub fn internal(message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(message, detail, WispErrorKind::Internal)
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub fn kind(&self) -> WispErrorKind {
        self.kind
    }
}
