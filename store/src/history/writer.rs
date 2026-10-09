//! Append-only JSONL writer with rotation (LUM-034).

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use super::entry::QueryHistoryEntry;
use super::sanitize::sql_for_history;

pub const HISTORY_MAX_BYTES: u64 = 5 * 1024 * 1024;
const ROTATE_TARGET_BYTES: u64 = HISTORY_MAX_BYTES - 512 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum HistoryWriteError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Append one entry when history is enabled for the connection.
pub fn append_history_entry(
    path: &Path,
    enabled: bool,
    mut entry: QueryHistoryEntry,
) -> Result<(), HistoryWriteError> {
    if !enabled {
        return Ok(());
    }
    entry.sql = sql_for_history(&entry.sql);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(&entry)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    file.sync_data()?;
    rotate_if_needed(path)?;
    Ok(())
}

pub fn rotate_if_needed(path: &Path) -> io::Result<()> {
    let meta = std::fs::metadata(path)?;
    if meta.len() <= HISTORY_MAX_BYTES {
        return Ok(());
    }
    let content = std::fs::read(path)?;
    let mut start = 0usize;
    while start < content.len() && (content.len() - start) as u64 > ROTATE_TARGET_BYTES {
        let Some(rel) = content[start..].iter().position(|&b| b == b'\n') else {
            start = content.len();
            break;
        };
        start += rel + 1;
    }
    if start >= content.len() {
        File::create(path)?.sync_data()?;
    } else {
        std::fs::write(path, &content[start..])?;
    }
    Ok(())
}
