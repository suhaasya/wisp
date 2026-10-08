//! Thread-safe access to saved connections + secrets.

use std::sync::Mutex;

use thiserror::Error;
use wisp_store::{
    paths::WispPaths, ConnectionId, ConnectionProfile, ConnectionStore, ConnectionsLoadError,
    Secret, SecretKind, SharedSecretStore,
};

#[derive(Debug, Error)]
pub enum ConnectionHubError {
    #[error(transparent)]
    Load(#[from] ConnectionsLoadError),
    #[error("{0}")]
    Store(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Shared connection persistence used by the UI and session layer.
pub struct ConnectionHub {
    store: Mutex<ConnectionStore>,
    secrets: SharedSecretStore,
}

impl ConnectionHub {
    pub fn load(paths: WispPaths, secrets: SharedSecretStore) -> Result<Self, ConnectionHubError> {
        let store = ConnectionStore::load_from_paths(paths)?;
        Ok(Self {
            store: Mutex::new(store),
            secrets,
        })
    }

    pub fn from_store(store: ConnectionStore, secrets: SharedSecretStore) -> Self {
        Self {
            store: Mutex::new(store),
            secrets,
        }
    }

    pub fn empty_at(path: std::path::PathBuf, secrets: SharedSecretStore) -> Self {
        Self {
            store: Mutex::new(ConnectionStore::empty_at(path)),
            secrets,
        }
    }

    pub fn with_lock<R>(&self, f: impl FnOnce(&mut ConnectionStore) -> R) -> R {
        let mut guard = self.store.lock().expect("connection store lock");
        f(&mut guard)
    }

    pub fn profiles(&self) -> Vec<ConnectionProfile> {
        self.with_lock(|s| s.profiles().to_vec())
    }

    pub fn folders(&self) -> Vec<wisp_store::ConnectionFolder> {
        self.with_lock(|s| s.folders().to_vec())
    }

    pub fn get(&self, id: ConnectionId) -> Option<ConnectionProfile> {
        self.with_lock(|s| s.get(id).cloned())
    }

    pub fn update(&self, profile: ConnectionProfile) -> Result<(), ConnectionHubError> {
        self.with_lock(|s| {
            s.update(profile)
                .map_err(|e| ConnectionHubError::Store(e.to_string()))
        })
    }

    pub fn save_profile(
        &self,
        profile: ConnectionProfile,
        password: Option<&str>,
        persist_password: bool,
    ) -> Result<ConnectionId, ConnectionHubError> {
        let id = profile.id;
        let exists = self.get(id).is_some();
        if exists {
            self.update(profile)?;
        } else {
            self.create(profile)?;
        }
        let key = id.to_string();
        if persist_password {
            if let Some(password) = password.filter(|p| !p.is_empty()) {
                self.secrets
                    .set(&key, SecretKind::Password, &Secret::from_utf8(password))
                    .map_err(|e| ConnectionHubError::Store(e.to_string()))?;
            }
        } else {
            self.secrets
                .delete(&key, SecretKind::Password)
                .map_err(|e| ConnectionHubError::Store(e.to_string()))?;
        }
        Ok(id)
    }

    pub fn create(&self, profile: ConnectionProfile) -> Result<(), ConnectionHubError> {
        self.with_lock(|s| {
            s.create(profile)
                .map_err(|e| ConnectionHubError::Store(e.to_string()))
        })
    }

    pub fn delete(&self, id: ConnectionId) -> Result<(), ConnectionHubError> {
        self.with_lock(|s| {
            s.delete(id, self.secrets.as_ref())
                .map_err(|e| ConnectionHubError::Store(e.to_string()))
        })
    }

    pub fn duplicate(&self, id: ConnectionId) -> Result<ConnectionId, ConnectionHubError> {
        self.with_lock(|s| {
            s.duplicate(id, self.secrets.as_ref())
                .map_err(|e| ConnectionHubError::Store(e.to_string()))
        })
    }

    pub fn move_to_folder(
        &self,
        id: ConnectionId,
        folder_id: Option<ConnectionId>,
    ) -> Result<(), ConnectionHubError> {
        self.with_lock(|s| {
            s.move_to_folder(id, folder_id)
                .map_err(|e| ConnectionHubError::Store(e.to_string()))
        })
    }

    pub fn touch_connect(&self, id: ConnectionId) -> Result<(), ConnectionHubError> {
        self.with_lock(|s| {
            s.touch_last_used(id)
                .map_err(|e| ConnectionHubError::Store(e.to_string()))
        })
    }

    pub fn session_secrets(&self, id: ConnectionId) -> crate::session::SessionSecrets {
        crate::session::SessionSecrets::load(&self.secrets, &id.to_string())
    }

    pub fn create_folder(&self, name: impl Into<String>) -> ConnectionId {
        self.with_lock(|s| s.create_folder(name))
    }
}
