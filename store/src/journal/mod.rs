//! Crash-safe session journal (LUM-035).

mod io;
mod schema;
mod writer;

pub use io::{read_journal, remove_journal, write_journal_atomic};
pub use schema::{
    json_has_credential_keys, JournalCellEdit, JournalInsertedRow, JournalRowKey, JournalTab,
    JournalTabKind, JournalWorkspace, StagedCellEditRow, StagedGridSnapshot, WindowJournal,
    JOURNAL_VERSION,
};
pub use writer::JournalWriter;

use std::path::{Path, PathBuf};

/// List recoverable journal files (newest first by `updated_unix_ms`).
pub fn list_pending_journals(dir: &Path) -> std::io::Result<Vec<(PathBuf, WindowJournal)>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".tmp"))
        {
            continue;
        }
        if let Ok(Some(doc)) = read_journal(&path) {
            if doc.workspace.is_some() {
                out.push((path, doc));
            }
        }
    }
    out.sort_by(|a, b| b.1.updated_unix_ms.cmp(&a.1.updated_unix_ms));
    Ok(out)
}

pub fn discard_pending(paths: &[PathBuf]) -> std::io::Result<()> {
    for path in paths {
        remove_journal(path)?;
    }
    Ok(())
}
