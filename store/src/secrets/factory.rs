//! Select keychain vs encrypted vault vs mock backend.

use std::sync::Arc;

use super::{
    error::SecretError,
    file_vault::FileVaultStore,
    keyring_store::KeyringSecretStore,
    secret::Secret,
    store::{MockSecretStore, SharedSecretStore},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretBackendKind {
    Keyring,
    FileVault,
    Mock,
}

pub struct OpenSecretStoreOptions<'a> {
    pub paths: &'a crate::paths::WispPaths,
    /// Required when creating or unlocking the encrypted vault.
    pub master_password: Option<&'a Secret>,
}

/// Open the best available secret backend for this host.
pub fn open_secret_store(opts: OpenSecretStoreOptions<'_>) -> Result<(SharedSecretStore, SecretBackendKind), SecretError> {
    if std::env::var_os("WISP_SECRET_BACKEND").as_deref() == Some("mock".as_ref()) {
        return Ok((
            Arc::new(MockSecretStore::new()) as SharedSecretStore,
            SecretBackendKind::Mock,
        ));
    }

    if std::env::var_os("WISP_SECRET_BACKEND").as_deref() != Some("file".as_ref())
        && KeyringSecretStore::probe().is_ok()
    {
        return Ok((
            Arc::new(KeyringSecretStore::new()) as SharedSecretStore,
            SecretBackendKind::Keyring,
        ));
    }

    let vault_path = FileVaultStore::vault_path(opts.paths);
    if vault_path.is_file() {
        let master = opts
            .master_password
            .ok_or(SecretError::MasterPasswordRequired)?;
        let vault = FileVaultStore::unlock(vault_path, master)?;
        return Ok((
            Arc::new(vault) as SharedSecretStore,
            SecretBackendKind::FileVault,
        ));
    }

    if let Some(master) = opts.master_password {
        let vault = FileVaultStore::create(vault_path, master)?;
        return Ok((
            Arc::new(vault) as SharedSecretStore,
            SecretBackendKind::FileVault,
        ));
    }

    Err(SecretError::MasterPasswordRequired)
}
