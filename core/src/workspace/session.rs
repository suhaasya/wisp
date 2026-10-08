//! Persist open tabs per connection (metadata only — no query re-run on restore).

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use wisp_store::{ConnectionId, WispPaths};

const FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedWorkspaceTabKind {
    TableData,
    TableStructure,
    Query,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedWorkspaceTab {
    pub title: String,
    pub kind: SavedWorkspaceTabKind,
    pub pinned: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceTabState {
    pub active_tab: usize,
    pub next_tab_id: u32,
    pub tabs: Vec<SavedWorkspaceTab>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceSessionsFile {
    version: u32,
    #[serde(default)]
    by_connection: HashMap<String, WorkspaceTabState>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceSessionStore {
    path: PathBuf,
    by_connection: HashMap<ConnectionId, WorkspaceTabState>,
}

impl WorkspaceSessionStore {
    pub fn load(paths: &WispPaths) -> Self {
        let path = paths.data_dir().join("workspace_sessions.json");
        let by_connection = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<WorkspaceSessionsFile>(&raw).ok())
            .filter(|f| f.version == FILE_VERSION)
            .map(|f| {
                f.by_connection
                    .into_iter()
                    .filter_map(|(k, v)| {
                        uuid::Uuid::parse_str(&k)
                            .ok()
                            .map(|u| (ConnectionId(u), v))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self { path, by_connection }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let by_connection: HashMap<String, WorkspaceTabState> = self
            .by_connection
            .iter()
            .map(|(id, state)| (id.0.to_string(), state.clone()))
            .collect();
        let file = WorkspaceSessionsFile {
            version: FILE_VERSION,
            by_connection,
        };
        let json = serde_json::to_string_pretty(&file).map_err(std::io::Error::other)?;
        fs::write(&self.path, json)
    }

    pub fn get(&self, id: ConnectionId) -> Option<&WorkspaceTabState> {
        self.by_connection.get(&id)
    }

    pub fn set(&mut self, id: ConnectionId, state: WorkspaceTabState) {
        if state.tabs.is_empty() {
            self.by_connection.remove(&id);
        } else {
            self.by_connection.insert(id, state);
        }
    }
}
