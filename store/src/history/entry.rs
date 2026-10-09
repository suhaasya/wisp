//! One append-only history record (LUM-034).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryHistoryStatus {
    Ok,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub sql: String,
    pub unix_ms: i64,
    pub duration_ms: u64,
    #[serde(default)]
    pub rows: Option<u64>,
    pub status: QueryHistoryStatus,
}

impl QueryHistoryEntry {
    pub fn new(
        sql: impl Into<String>,
        unix_ms: i64,
        duration_ms: u64,
        rows: Option<u64>,
        status: QueryHistoryStatus,
    ) -> Self {
        Self {
            sql: sql.into(),
            unix_ms,
            duration_ms,
            rows,
            status,
        }
    }
}
