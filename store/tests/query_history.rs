//! History rotation, corruption recovery, and search perf (LUM-034).

use std::time::Instant;

use tempfile::tempdir;
use wisp_store::{
    append_history_entry, count_valid_entries, rotate_if_needed, search_history_file,
    QueryHistoryEntry, QueryHistoryStatus, HISTORY_MAX_BYTES,
};

#[test]
fn disabled_history_writes_nothing() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("h.jsonl");
    append_history_entry(
        &path,
        false,
        QueryHistoryEntry::new("SELECT 1", 0, 1, Some(1), QueryHistoryStatus::Ok),
    )
    .expect("append");
    assert!(!path.exists());
}

#[test]
fn rotation_drops_oldest_lines() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("h.jsonl");
    let payload = "x".repeat(1500);
    for i in 0..4000 {
        append_history_entry(
            &path,
            true,
            QueryHistoryEntry::new(format!("SELECT {payload} FROM t WHERE id = {i}"), i, 1, None, QueryHistoryStatus::Ok),
        )
        .expect("append");
    }
    let len = std::fs::metadata(&path).expect("meta").len();
    assert!(len <= HISTORY_MAX_BYTES);
    let count = count_valid_entries(&path).expect("count");
    assert!(count > 0);
    assert!(count < 4000, "expected rotation to drop entries, got {count}");
}

#[test]
fn corrupted_lines_skipped_in_search() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("h.jsonl");
    std::fs::write(
        &path,
        "{bad json}\n{\"sql\":\"SELECT 1\",\"unix_ms\":1,\"duration_ms\":2,\"status\":\"ok\"}\n",
    )
    .expect("write");
    let hits = search_history_file(&path, "", 10).expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].sql, "SELECT 1");
}

#[test]
fn search_50k_entries_under_100ms() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("h.jsonl");
    let mut buf = String::new();
    for i in 0..50_000 {
        buf.push_str(&format!(
            "{{\"sql\":\"SELECT {i} FROM items WHERE k = 'v'\",\"unix_ms\":{i},\"duration_ms\":1,\"status\":\"ok\"}}\n"
        ));
    }
    std::fs::write(&path, buf).expect("write");
    let started = Instant::now();
    let hits = search_history_file(&path, "SELECT 499", 50).expect("search");
    let elapsed = started.elapsed();
    if cfg!(debug_assertions) {
        assert!(
            elapsed.as_millis() < 500,
            "debug search took {:?} for 50k lines",
            elapsed
        );
    } else {
        assert!(
            elapsed.as_millis() < 100,
            "release search took {:?} for 50k lines",
            elapsed
        );
    }
    assert!(!hits.is_empty());
}

#[test]
fn rotate_if_needed_trims_file() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("h.jsonl");
    let line = format!("{{\"sql\":\"{}\",\"unix_ms\":0,\"duration_ms\":0,\"status\":\"ok\"}}\n", "a".repeat(2000));
    let mut data = String::new();
    while data.len() < (HISTORY_MAX_BYTES + 1024) as usize {
        data.push_str(&line);
    }
    std::fs::write(&path, data).expect("write");
    rotate_if_needed(&path).expect("rotate");
    assert!(std::fs::metadata(&path).expect("meta").len() <= HISTORY_MAX_BYTES);
}
