//! Crash journal persistence (LUM-035).

use std::thread;
use std::time::Duration;

use uuid::Uuid;
use wisp_store::{
    json_has_credential_keys, list_pending_journals, read_journal, write_journal_atomic,
    JournalTab, JournalTabKind, JournalWorkspace, JournalWriter, WindowJournal, WispPaths,
};

#[test]
fn atomic_write_and_read_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let paths = WispPaths::from_dirs(dir.path().join("cfg"), dir.path().join("data"), dir.path().join("cache"));
    let path = paths.journal_file(Uuid::new_v4());
    let doc = WindowJournal::new(
        Uuid::new_v4(),
        Some(JournalWorkspace {
            connection_id: Uuid::new_v4(),
            active_tab: 0,
            next_tab_id: 2,
            tabs: vec![JournalTab {
                tab_id: 1,
                title: "Query 1".into(),
                kind: JournalTabKind::Query,
                pinned: false,
                query_sql: Some("SELECT 1".into()),
                staged_grid: None,
            }],
        }),
    );
    write_journal_atomic(&path, &doc).unwrap();
    let loaded = read_journal(&path).unwrap().expect("journal");
    assert_eq!(loaded, doc);
}

#[test]
fn crash_simulation_debounce_then_abandon() {
    let dir = tempfile::tempdir().unwrap();
    let paths = WispPaths::from_dirs(dir.path().join("cfg"), dir.path().join("data"), dir.path().join("cache"));
    let window_id = Uuid::new_v4();
    let path = paths.journal_file(window_id);
    let writer = JournalWriter::spawn(path.clone(), window_id);
    writer.schedule(WindowJournal::new(
        window_id,
        Some(JournalWorkspace {
            connection_id: Uuid::new_v4(),
            active_tab: 0,
            next_tab_id: 1,
            tabs: vec![JournalTab {
                tab_id: 1,
                title: "q".into(),
                kind: JournalTabKind::Query,
                pinned: false,
                query_sql: Some("SELECT crash_recovery_marker".into()),
                staged_grid: None,
            }],
        }),
    ));
    thread::sleep(Duration::from_millis(1200));
    // Simulate kill -9: drop without clear_on_exit.
    drop(writer);
    let pending = list_pending_journals(&paths.journal_dir()).unwrap();
    assert_eq!(pending.len(), 1);
    let sql = pending[0]
        .1
        .workspace
        .as_ref()
        .unwrap()
        .tabs[0]
        .query_sql
        .as_deref()
        .unwrap();
    assert_eq!(sql, "SELECT crash_recovery_marker");
}

#[test]
fn clean_exit_clears_journal() {
    let dir = tempfile::tempdir().unwrap();
    let paths = WispPaths::from_dirs(dir.path().join("cfg"), dir.path().join("data"), dir.path().join("cache"));
    let writer = JournalWriter::spawn_for_window(&paths);
    writer.schedule(WindowJournal::new(
        writer.window_id(),
        Some(JournalWorkspace {
            connection_id: Uuid::new_v4(),
            active_tab: 0,
            next_tab_id: 1,
            tabs: vec![JournalTab {
                tab_id: 1,
                title: "q".into(),
                kind: JournalTabKind::Query,
                pinned: false,
                query_sql: Some("SELECT 1".into()),
                staged_grid: None,
            }],
        }),
    ));
    thread::sleep(Duration::from_millis(1200));
    writer.clear_on_exit();
    assert!(list_pending_journals(&paths.journal_dir()).unwrap().is_empty());
}

#[test]
fn corrupt_journal_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let paths = WispPaths::from_dirs(dir.path().join("cfg"), dir.path().join("data"), dir.path().join("cache"));
    let path = paths.journal_file(Uuid::new_v4());
    std::fs::create_dir_all(paths.journal_dir()).unwrap();
    std::fs::write(&path, b"{not json").unwrap();
    assert!(read_journal(&path).unwrap().is_none());
    assert!(list_pending_journals(&paths.journal_dir()).unwrap().is_empty());
}

#[test]
fn journal_json_never_contains_password_keys() {
    let doc = WindowJournal::new(
        Uuid::new_v4(),
        Some(JournalWorkspace {
            connection_id: Uuid::new_v4(),
            active_tab: 0,
            next_tab_id: 1,
            tabs: vec![JournalTab {
                tab_id: 1,
                title: "users".into(),
                kind: JournalTabKind::Query,
                pinned: false,
                query_sql: Some("-- user typed: my password is in a comment".into()),
                staged_grid: None,
            }],
        }),
    );
    let json = serde_json::to_string(&doc).unwrap();
    assert!(
        !json_has_credential_keys(&json),
        "credential-like keys in journal: {json}"
    );
}
