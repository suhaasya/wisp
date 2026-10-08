//! Secret storage errors.

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecretError {
    #[error("secret not found")]
    NotFound,
    #[error("secrets are not stored for this connection (ask every time)")]
    AskEveryTime,
    #[error("master password required to unlock the encrypted secret vault")]
    MasterPasswordRequired,
    #[error("incorrect master password")]
    BadMasterPassword,
    #[error("OS keychain unavailable; use encrypted vault or mock backend")]
    KeychainUnavailable,
    #[error("invalid secret vault file")]
    CorruptVault,
    #[error("secret storage backend error: {0}")]
    Backend(String),
}

impl From<keyring::Error> for SecretError {
    fn from(value: keyring::Error) -> Self {
        match value {
            keyring::Error::NoEntry => Self::NotFound,
            other => Self::Backend(other.to_string()),
        }
    }
}
