//! `SecretStore` trait and policy-aware wrapper.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};

use super::{
    error::SecretError,
    kind::{storage_key, SecretKind},
    policy::ConnectionSecretPolicy,
    secret::Secret,
};

pub trait SecretStore: Send + Sync {
    fn get(&self, connection_id: &str, kind: SecretKind) -> Result<Secret, SecretError>;
    fn set(&self, connection_id: &str, kind: SecretKind, value: &Secret) -> Result<(), SecretError>;
    fn delete(&self, connection_id: &str, kind: SecretKind) -> Result<(), SecretError>;
    fn delete_connection(&self, connection_id: &str) -> Result<(), SecretError> {
        for kind in SecretKind::ALL {
            let _ = self.delete(connection_id, kind);
        }
        Ok(())
    }
}

/// Per-connection persistence policy (e.g. ask every time → nothing stored).
#[derive(Default)]
pub struct PolicySecretStore<S: SecretStore> {
    inner: S,
    policies: RwLock<HashMap<String, ConnectionSecretPolicy>>,
}

impl<S: SecretStore> PolicySecretStore<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            policies: RwLock::new(HashMap::new()),
        }
    }

    pub fn set_policy(&self, connection_id: impl Into<String>, policy: ConnectionSecretPolicy) {
        self.policies
            .write()
            .expect("policy lock")
            .insert(connection_id.into(), policy);
    }

    pub fn policy(&self, connection_id: &str) -> ConnectionSecretPolicy {
        self.policies
            .read()
            .expect("policy lock")
            .get(connection_id)
            .copied()
            .unwrap_or_default()
    }

    fn ensure_stored(&self, connection_id: &str) -> Result<(), SecretError> {
        if self.policy(connection_id) == ConnectionSecretPolicy::AskEveryTime {
            return Err(SecretError::AskEveryTime);
        }
        Ok(())
    }
}

impl<S: SecretStore> SecretStore for PolicySecretStore<S> {
    fn get(&self, connection_id: &str, kind: SecretKind) -> Result<Secret, SecretError> {
        self.ensure_stored(connection_id)?;
        self.inner.get(connection_id, kind)
    }

    fn set(&self, connection_id: &str, kind: SecretKind, value: &Secret) -> Result<(), SecretError> {
        if self.policy(connection_id) == ConnectionSecretPolicy::AskEveryTime {
            return Ok(());
        }
        self.inner.set(connection_id, kind, value)
    }

    fn delete(&self, connection_id: &str, kind: SecretKind) -> Result<(), SecretError> {
        self.inner.delete(connection_id, kind)
    }

    fn delete_connection(&self, connection_id: &str) -> Result<(), SecretError> {
        self.policies
            .write()
            .expect("policy lock")
            .remove(connection_id);
        self.inner.delete_connection(connection_id)
    }
}

/// In-memory mock for tests and CI without a keychain.
#[derive(Default)]
pub struct MockSecretStore {
    entries: Mutex<HashMap<String, Secret>>,
}

impl MockSecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MockSecretStore {
    fn get(&self, connection_id: &str, kind: SecretKind) -> Result<Secret, SecretError> {
        let key = storage_key(connection_id, kind);
        self.entries
            .lock()
            .expect("mock lock")
            .get(&key)
            .cloned()
            .ok_or(SecretError::NotFound)
    }

    fn set(&self, connection_id: &str, kind: SecretKind, value: &Secret) -> Result<(), SecretError> {
        let key = storage_key(connection_id, kind);
        self.entries
            .lock()
            .expect("mock lock")
            .insert(key, value.clone());
        Ok(())
    }

    fn delete(&self, connection_id: &str, kind: SecretKind) -> Result<(), SecretError> {
        let key = storage_key(connection_id, kind);
        self.entries.lock().expect("mock lock").remove(&key);
        Ok(())
    }
}

pub type SharedSecretStore = Arc<dyn SecretStore>;
