//! OS keychain and encrypted vault secret storage (LUM-009).

mod blocking;
mod error;
mod factory;
mod file_vault;
mod keyring_store;
mod kind;
mod policy;
mod secret;
mod store;

pub use blocking::BlockingSecretStore;
pub use error::SecretError;
pub use factory::{open_secret_store, OpenSecretStoreOptions, SecretBackendKind};
pub use file_vault::FileVaultStore;
pub use keyring_store::KeyringSecretStore;
pub use kind::{storage_key, SecretKind};
pub use policy::ConnectionSecretPolicy;
pub use secret::Secret;
pub use store::{MockSecretStore, PolicySecretStore, SecretStore, SharedSecretStore};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::WispPaths;

    #[test]
    fn mock_roundtrip_and_delete_connection() {
        let store = MockSecretStore::new();
        let value = Secret::from_utf8("hunter2");
        store
            .set("conn-1", SecretKind::Password, &value)
            .expect("set");
        let read = store.get("conn-1", SecretKind::Password).expect("get");
        assert_eq!(read, value);
        store.delete_connection("conn-1").expect("delete");
        assert_eq!(
            store.get("conn-1", SecretKind::Password),
            Err(SecretError::NotFound)
        );
    }

    #[test]
    fn ask_every_time_skips_storage() {
        let inner = MockSecretStore::new();
        let store = PolicySecretStore::new(inner);
        store.set_policy("conn-a", ConnectionSecretPolicy::AskEveryTime);
        store
            .set("conn-a", SecretKind::Password, &Secret::from_utf8("x"))
            .expect("set noop");
        assert_eq!(
            store.get("conn-a", SecretKind::Password),
            Err(SecretError::AskEveryTime)
        );
    }

    #[test]
    fn file_vault_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = WispPaths::from_dirs(
            dir.path().join("config"),
            dir.path().join("data"),
            dir.path().join("cache"),
        );
        std::fs::create_dir_all(paths.data_dir()).expect("data dir");
        let vault_path = FileVaultStore::vault_path(&paths);
        let master = Secret::from_utf8("correct-horse-battery-staple");
        let store = FileVaultStore::create(vault_path.clone(), &master).expect("create");
        store
            .set("pg-main", SecretKind::Password, &Secret::from_utf8("s3cret"))
            .expect("set");
        drop(store);
        let unlocked = FileVaultStore::unlock(vault_path, &master).expect("unlock");
        let got = unlocked
            .get("pg-main", SecretKind::Password)
            .expect("get");
        assert_eq!(got.expose_str(), "s3cret");
    }

    #[test]
    fn open_store_uses_mock_when_env_set() {
        std::env::set_var("WISP_SECRET_BACKEND", "mock");
        let paths = WispPaths::resolve();
        let (store, kind) = open_secret_store(OpenSecretStoreOptions {
            paths: &paths,
            master_password: None,
        })
        .expect("open mock");
        assert_eq!(kind, SecretBackendKind::Mock);
        store
            .set("x", SecretKind::Token, &Secret::from_utf8("t"))
            .expect("set");
        std::env::remove_var("WISP_SECRET_BACKEND");
    }
}
