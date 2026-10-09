//! Saved SQL snippets (LUM-034).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ConnectionId;

pub const SNIPPETS_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetsFile {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
}

fn default_version() -> u32 {
    SNIPPETS_VERSION
}

impl Default for SnippetsFile {
    fn default() -> Self {
        Self {
            version: SNIPPETS_VERSION,
            snippets: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snippet {
    pub id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<ConnectionId>,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub sql: String,
    #[serde(default)]
    pub created_unix: i64,
}

impl Snippet {
    pub fn new(
        name: impl Into<String>,
        sql: impl Into<String>,
        connection_id: Option<ConnectionId>,
        tags: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            connection_id,
            name: name.into(),
            tags,
            sql: sql.into(),
            created_unix: chrono_unix_ms(),
        }
    }
}

fn chrono_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
