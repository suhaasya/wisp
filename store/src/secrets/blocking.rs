//! Run blocking keychain / vault I/O off the UI thread (std thread pool of one).

use std::sync::Arc;

use super::{
    error::SecretError,
    kind::SecretKind,
    secret::Secret,
    store::SecretStore,
};

/// Wraps a [`SecretStore`] so every call runs on a background thread.
pub struct BlockingSecretStore {
    inner: Arc<dyn SecretStore>,
}

impl BlockingSecretStore {
    pub fn new(inner: Arc<dyn SecretStore>) -> Self {
        Self { inner }
    }

    pub fn get(&self, connection_id: &str, kind: SecretKind) -> Result<Secret, SecretError> {
        let inner = self.inner.clone();
        let connection_id = connection_id.to_string();
        run_blocking(move || inner.get(&connection_id, kind))
    }

    pub fn set(
        &self,
        connection_id: &str,
        kind: SecretKind,
        value: Secret,
    ) -> Result<(), SecretError> {
        let inner = self.inner.clone();
        let connection_id = connection_id.to_string();
        run_blocking(move || inner.set(&connection_id, kind, &value))
    }

    pub fn delete(&self, connection_id: &str, kind: SecretKind) -> Result<(), SecretError> {
        let inner = self.inner.clone();
        let connection_id = connection_id.to_string();
        run_blocking(move || inner.delete(&connection_id, kind))
    }

    pub fn delete_connection(&self, connection_id: &str) -> Result<(), SecretError> {
        let inner = self.inner.clone();
        let connection_id = connection_id.to_string();
        run_blocking(move || inner.delete_connection(&connection_id))
    }
}

fn run_blocking<T, F>(f: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::spawn(f)
        .join()
        .expect("blocking secret store thread")
}
