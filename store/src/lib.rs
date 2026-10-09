//! Local configuration, credential storage, and command/query history.
//!
//! Owns durable and semi-durable app data on disk or OS keychains. Must not depend on
//! drivers, transport, UI, or session orchestration in `wisp-core` (avoid cycles).

pub mod connections;
pub mod history;
pub mod journal;
pub mod paths;
pub mod snippets;
pub mod secrets;
pub mod settings;
pub mod window;

pub use connections::{
    parse_connections_toml, ConnectionEngine, ConnectionFolder, ConnectionId, ConnectionProfile,
    ConnectionStore, ConnectionStoreError, ConnectionsFile, ConnectionsLoadError,
    ConnectionsParseError, EnvironmentTag, SslMode, SslSettings, SslTrustStore, SshAuthMethod,
    SshSettings,
    TransportKind,
    CONNECTIONS_VERSION,
};
pub use history::{
    append_history_entry, count_valid_entries, rotate_if_needed, search_history_file,
    sql_for_history, HistoryWriteError, QueryHistoryEntry, QueryHistoryStatus,
    HISTORY_MAX_BYTES,
};
pub use journal::{
    discard_pending, json_has_credential_keys, list_pending_journals, read_journal,
    remove_journal, write_journal_atomic, JournalCellEdit, JournalInsertedRow, JournalRowKey,
    JournalTab, JournalTabKind, JournalWorkspace, JournalWriter, StagedCellEditRow,
    StagedGridSnapshot, WindowJournal, JOURNAL_VERSION,
};
pub use paths::WispPaths;
pub use snippets::{Snippet, SnippetStore, SnippetStoreError, SnippetsFile, SNIPPETS_VERSION};
pub use secrets::{
    open_secret_store, BlockingSecretStore, ConnectionSecretPolicy, MockSecretStore,
    OpenSecretStoreOptions, Secret, SecretBackendKind, SecretError, SecretKind, SecretStore,
    SharedSecretStore,
};
pub use settings::{
    parse_settings_toml, spawn_settings_watcher, AppearanceSettings, DensitySetting,
    GridSettings, KeybindingEntry, KeybindingsSettings, MonoFontSetting, ResultsSettings,
    Settings, SettingsParseError, SettingsStore, SettingsWatchEvent, TelemetrySettings,
    ThemeModeSetting, UiFontSetting, SETTINGS_VERSION,
};

/// Placeholder until store implementations land in later milestones.
pub const CRATE_MARKER: &str = "wisp-store";

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(super::CRATE_MARKER, "wisp-store");
    }
}
