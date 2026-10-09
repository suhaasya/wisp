//! Load/save `snippets.toml` (LUM-034).

use std::path::{Path, PathBuf};

use super::schema::{Snippet, SnippetsFile, SNIPPETS_VERSION};
use crate::ConnectionId;

#[derive(Debug, thiserror::Error)]
pub enum SnippetStoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse: {0}")]
    Parse(String),
}

pub struct SnippetStore {
    path: PathBuf,
    file: SnippetsFile,
}

impl SnippetStore {
    pub fn load(path: PathBuf) -> Result<Self, SnippetStoreError> {
        let file = if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            toml::from_str(&raw).map_err(|e| SnippetStoreError::Parse(e.to_string()))?
        } else {
            SnippetsFile::default()
        };
        Ok(Self { path, file })
    }

    pub fn snippets_for_connection(&self, id: ConnectionId) -> Vec<&Snippet> {
        self.file
            .snippets
            .iter()
            .filter(|s| s.connection_id.is_none() || s.connection_id == Some(id))
            .collect()
    }

    pub fn all(&self) -> &[Snippet] {
        &self.file.snippets
    }

    pub fn upsert(&mut self, snippet: Snippet) {
        if let Some(ix) = self.file.snippets.iter().position(|s| s.id == snippet.id) {
            self.file.snippets[ix] = snippet;
        } else {
            self.file.snippets.push(snippet);
        }
    }

    pub fn remove(&mut self, id: uuid::Uuid) {
        self.file.snippets.retain(|s| s.id != id);
    }

    pub fn save(&mut self) -> Result<(), SnippetStoreError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.file.version = SNIPPETS_VERSION;
        let raw = toml::to_string_pretty(&self.file)
            .map_err(|e| SnippetStoreError::Parse(e.to_string()))?;
        std::fs::write(&self.path, raw)?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
