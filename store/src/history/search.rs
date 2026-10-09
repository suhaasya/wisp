//! Streamed history search without loading the full file (LUM-034).

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use super::entry::QueryHistoryEntry;

/// Scan the JSONL file once; skips corrupted lines. Returns newest matches first.
pub fn search_history_file(
    path: &Path,
    query: &str,
    limit: usize,
) -> std::io::Result<Vec<QueryHistoryEntry>> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let reader = BufReader::new(file);
    let needle = query.trim().to_ascii_lowercase();
    let mut window: Vec<QueryHistoryEntry> = Vec::with_capacity(limit.min(256));

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<QueryHistoryEntry>(trimmed) else {
            continue;
        };
        if !needle.is_empty() && !entry.sql.to_ascii_lowercase().contains(&needle) {
            continue;
        }
        window.push(entry);
        if window.len() > limit {
            window.remove(0);
        }
    }
    window.reverse();
    Ok(window)
}

/// Count valid lines (for tests).
pub fn count_valid_entries(path: &Path) -> std::io::Result<usize> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let reader = BufReader::new(file);
    let mut n = 0usize;
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        if serde_json::from_str::<QueryHistoryEntry>(line.trim()).is_ok() {
            n += 1;
        }
    }
    Ok(n)
}
