//! Local configuration, credential storage, and command/query history.
//!
//! Owns durable and semi-durable app data on disk or OS keychains. Must not depend on
//! drivers, transport, UI, or session orchestration in `wisp-core` (avoid cycles).

pub mod connections;
pub mod paths;
pub mod secrets;
pub mod settings;
pub mod window;

pub use connections::{
    parse_connections_toml, ConnectionEngine, ConnectionFolder, ConnectionId, ConnectionProfile,
    ConnectionStore, ConnectionStoreError, ConnectionsFile, ConnectionsLoadError,
    ConnectionsParseError, EnvironmentTag, SslMode, SslSettings, SshSettings, TransportKind,
    CONNECTIONS_VERSION,
};
pub use paths::WispPaths;
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
