//! OS keychain integration (run locally or when `WISP_KEYRING_IT=1` in CI).

use wisp_store::secrets::{
    KeyringSecretStore, Secret, SecretKind, SecretStore,
};

#[test]
fn keyring_roundtrip_when_enabled() {
    if std::env::var_os("WISP_KEYRING_IT").is_none() {
        eprintln!("skipping keyring integration (set WISP_KEYRING_IT=1 to run)");
        return;
    }
    let store = KeyringSecretStore::new();
    let id = format!("it-{}", std::process::id());
    store
        .set(&id, SecretKind::Password, &Secret::from_utf8("integration-test"))
        .expect("set");
    let value = store.get(&id, SecretKind::Password).expect("get");
    assert_eq!(value.expose_str(), "integration-test");
    store.delete_connection(&id).expect("delete");
    assert!(matches!(
        store.get(&id, SecretKind::Password),
        Err(wisp_store::secrets::SecretError::NotFound)
    ));
}
