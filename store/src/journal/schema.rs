//! Crash-recovery journal document (LUM-035). No connection secrets.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const JOURNAL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowJournal {
    pub version: u32,
    pub window_id: Uuid,
    pub updated_unix_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<JournalWorkspace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalWorkspace {
    pub connection_id: Uuid,
    pub active_tab: usize,
    pub next_tab_id: u32,
    pub tabs: Vec<JournalTab>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalTabKind {
    TableData,
    TableStructure,
    Query,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalTab {
    pub tab_id: u32,
    pub title: String,
    pub kind: JournalTabKind,
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_sql: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staged_grid: Option<StagedGridSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedGridSnapshot {
    pub pk_columns: Vec<String>,
    pub cell_edits: Vec<StagedCellEditRow>,
    pub deleted: Vec<JournalRowKey>,
    pub inserts: Vec<JournalInsertedRow>,
    pub next_temp_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedCellEditRow {
    pub key: JournalRowKey,
    pub columns: Vec<(String, JournalCellEdit)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalInsertedRow {
    pub temp_id: u64,
    pub cells: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JournalRowKey {
    Pk { columns: Vec<(String, String)> },
    Insert { temp_id: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JournalCellEdit {
    Set { value: String },
    Null,
    Default,
}

impl WindowJournal {
    pub fn new(window_id: Uuid, workspace: Option<JournalWorkspace>) -> Self {
        Self {
            version: JOURNAL_VERSION,
            window_id,
            updated_unix_ms: unix_ms_now(),
            workspace,
        }
    }
}

fn unix_ms_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Ensures serialized journals never carry connection credential fields.
pub fn json_has_credential_keys(json: &str) -> bool {
    let lower = json.to_ascii_lowercase();
    lower.contains("\"password\"")
        || lower.contains("\"master_password\"")
        || lower.contains("\"secret\"")
        || lower.contains("\"private_key\"")
}
