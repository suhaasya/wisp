//! CRUD for saved connection profiles and folders.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::{
    paths::WispPaths,
    secrets::{SecretKind, SecretStore},
};

use super::{
    io::{backup_corrupt_file, write_connections_atomic},
    parse::{parse_connections_toml, ConnectionsParseError},
    schema::{ConnectionFolder, ConnectionId, ConnectionProfile, ConnectionsFile},
};

#[derive(Debug, Error)]
pub enum ConnectionsLoadError {
    #[error("{message}")]
    Corrupt {
        message: String,
        backup_path: PathBuf,
        detail: ConnectionsParseError,
    },
    #[error("could not read connections file: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum ConnectionStoreError {
    #[error("connection not found")]
    NotFound,
    #[error("connection id already exists")]
    DuplicateId,
    #[error("folder not found")]
    FolderNotFound,
    #[error("secret error: {0}")]
    Secret(String),
    #[error("could not write connections file: {0}")]
    Io(#[from] std::io::Error),
}

/// Saved connections (profiles + folders); secrets live in [`SecretStore`] only.
#[derive(Debug, Clone)]
pub struct ConnectionStore {
    path: PathBuf,
    file: ConnectionsFile,
}

impl ConnectionStore {
    pub fn load() -> Result<Self, ConnectionsLoadError> {
        Self::load_from_paths(WispPaths::resolve())
    }

    pub fn load_from_paths(paths: WispPaths) -> Result<Self, ConnectionsLoadError> {
        Self::load_from_path(paths.connections_toml())
    }

    pub fn load_from_path(path: PathBuf) -> Result<Self, ConnectionsLoadError> {
        if !path.is_file() {
            return Ok(Self {
                path,
                file: ConnectionsFile::default(),
            });
        }
        let text = std::fs::read_to_string(&path)?;
        match parse_connections_toml(&text) {
            Ok(file) => Ok(Self { path, file }),
            Err(detail) => {
                let backup_path = backup_corrupt_file(&path)?;
                let message = detail.user_message();
                Err(ConnectionsLoadError::Corrupt {
                    message,
                    backup_path,
                    detail,
                })
            }
        }
    }

    pub fn empty_at(path: PathBuf) -> Self {
        Self {
            path,
            file: ConnectionsFile::default(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file(&self) -> &ConnectionsFile {
        &self.file
    }

    pub fn profiles(&self) -> &[ConnectionProfile] {
        &self.file.profiles
    }

    pub fn folders(&self) -> &[ConnectionFolder] {
        &self.file.folders
    }

    pub fn folders_sorted(&self) -> Vec<&ConnectionFolder> {
        let mut out: Vec<_> = self.file.folders.iter().collect();
        out.sort_by_key(|f| f.order);
        out
    }

    pub fn get(&self, id: ConnectionId) -> Option<&ConnectionProfile> {
        self.file.profiles.iter().find(|p| p.id == id)
    }

    pub fn save(&self) -> std::io::Result<()> {
        write_connections_atomic(&self.path, &self.file)
    }

    pub fn create(&mut self, profile: ConnectionProfile) -> Result<(), ConnectionStoreError> {
        self.create_inner(profile)
    }

    fn create_inner(&mut self, profile: ConnectionProfile) -> Result<(), ConnectionStoreError> {
        if self.get(profile.id).is_some() {
            return Err(ConnectionStoreError::DuplicateId);
        }
        if let Some(folder_id) = profile.folder_id {
            self.ensure_folder(folder_id)?;
        }
        self.file.profiles.push(profile);
        self.save()?;
        Ok(())
    }

    pub fn update(&mut self, profile: ConnectionProfile) -> Result<(), ConnectionStoreError> {
        let Some(idx) = self.file.profiles.iter().position(|p| p.id == profile.id) else {
            return Err(ConnectionStoreError::NotFound);
        };
        if let Some(folder_id) = profile.folder_id {
            self.ensure_folder(folder_id)?;
        }
        self.file.profiles[idx] = profile;
        self.save()?;
        Ok(())
    }

    pub fn delete(
        &mut self,
        id: ConnectionId,
        secrets: &dyn SecretStore,
    ) -> Result<(), ConnectionStoreError> {
        let len_before = self.file.profiles.len();
        self.file.profiles.retain(|p| p.id != id);
        if self.file.profiles.len() == len_before {
            return Err(ConnectionStoreError::NotFound);
        }
        secrets
            .delete_connection(&id.to_string())
            .map_err(|e| ConnectionStoreError::Secret(e.to_string()))?;
        self.save()?;
        Ok(())
    }

    pub fn duplicate(
        &mut self,
        id: ConnectionId,
        secrets: &dyn SecretStore,
    ) -> Result<ConnectionId, ConnectionStoreError> {
        let source = self
            .get(id)
            .cloned()
            .ok_or(ConnectionStoreError::NotFound)?;
        let mut copy = source.clone();
        copy.id = ConnectionId::new_v7();
        copy.name = format!("{} copy", source.name);
        copy.last_used_unix = None;
        copy_secrets(secrets, &id, &copy.id)?;
        self.create_inner(copy.clone())?;
        Ok(copy.id)
    }

    pub fn move_to_folder(
        &mut self,
        id: ConnectionId,
        folder_id: Option<ConnectionId>,
    ) -> Result<(), ConnectionStoreError> {
        if let Some(fid) = folder_id {
            self.ensure_folder(fid)?;
        }
        let Some(idx) = self.file.profiles.iter().position(|p| p.id == id) else {
            return Err(ConnectionStoreError::NotFound);
        };
        self.file.profiles[idx].folder_id = folder_id;
        self.save()?;
        Ok(())
    }

    pub fn touch_last_used(&mut self, id: ConnectionId) -> Result<(), ConnectionStoreError> {
        let Some(idx) = self.file.profiles.iter().position(|p| p.id == id) else {
            return Err(ConnectionStoreError::NotFound);
        };
        self.file.profiles[idx].last_used_unix = Some(unix_now());
        self.save()?;
        Ok(())
    }

    pub fn create_folder(&mut self, name: impl Into<String>) -> ConnectionId {
        let order = self
            .file
            .folders
            .iter()
            .map(|f| f.order)
            .max()
            .map_or(0, |m| m + 1);
        let folder = ConnectionFolder {
            id: ConnectionId::new_v7(),
            name: name.into(),
            order,
        };
        let id = folder.id;
        self.file.folders.push(folder);
        let _ = self.save();
        id
    }

    pub fn rename_folder(
        &mut self,
        id: ConnectionId,
        name: impl Into<String>,
    ) -> Result<(), ConnectionStoreError> {
        let Some(folder) = self.file.folders.iter_mut().find(|f| f.id == id) else {
            return Err(ConnectionStoreError::FolderNotFound);
        };
        folder.name = name.into();
        self.save()?;
        Ok(())
    }

    pub fn reorder_folders(&mut self, ordered_ids: &[ConnectionId]) -> Result<(), ConnectionStoreError> {
        for (order, id) in ordered_ids.iter().enumerate() {
            let Some(folder) = self.file.folders.iter_mut().find(|f| f.id == *id) else {
                return Err(ConnectionStoreError::FolderNotFound);
            };
            folder.order = order as u32;
        }
        self.save()?;
        Ok(())
    }

    fn ensure_folder(&self, id: ConnectionId) -> Result<(), ConnectionStoreError> {
        if self.file.folders.iter().any(|f| f.id == id) {
            Ok(())
        } else {
            Err(ConnectionStoreError::FolderNotFound)
        }
    }
}

fn copy_secrets(
    secrets: &dyn SecretStore,
    from: &ConnectionId,
    to: &ConnectionId,
) -> Result<(), ConnectionStoreError> {
    let from = from.to_string();
    let to = to.to_string();
    for kind in SecretKind::ALL {
        if let Ok(value) = secrets.get(&from, kind) {
            secrets
                .set(&to, kind, &value)
                .map_err(|e| ConnectionStoreError::Secret(e.to_string()))?;
        }
    }
    Ok(())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
