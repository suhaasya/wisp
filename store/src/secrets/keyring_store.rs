//! OS keychain via the `keyring` crate.

use keyring::Entry;

use super::{
    error::SecretError,
    kind::{storage_key, SecretKind},
    secret::Secret,
    store::SecretStore,
};

const SERVICE: &str = "wisp";

pub struct KeyringSecretStore;

impl KeyringSecretStore {
    pub fn new() -> Self {
        Self
    }

    pub fn probe() -> Result<(), SecretError> {
        let probe_key = "probe/availability";
        let entry = Entry::new(SERVICE, probe_key).map_err(SecretError::from)?;
        entry
            .set_password("ok")
            .map_err(|e| SecretError::Backend(e.to_string()))?;
        entry.delete_credential().map_err(SecretError::from)?;
        Ok(())
    }

    fn entry(connection_id: &str, kind: SecretKind) -> Result<Entry, SecretError> {
        Entry::new(SERVICE, &storage_key(connection_id, kind)).map_err(SecretError::from)
    }
}

impl Default for KeyringSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, connection_id: &str, kind: SecretKind) -> Result<Secret, SecretError> {
        let entry = Self::entry(connection_id, kind)?;
        let value = entry.get_password()?;
        Ok(Secret::from(value))
    }

    fn set(&self, connection_id: &str, kind: SecretKind, value: &Secret) -> Result<(), SecretError> {
        let entry = Self::entry(connection_id, kind)?;
        entry
            .set_password(value.expose_str())
            .map_err(SecretError::from)
    }

    fn delete(&self, connection_id: &str, kind: SecretKind) -> Result<(), SecretError> {
        let entry = Self::entry(connection_id, kind)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(SecretError::from(err)),
        }
    }
}
