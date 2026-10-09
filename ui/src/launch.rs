//! Window launch/persistence DTOs (mapped from `wisp-store` in the binary crate).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use wisp_core::{
    ConnectionHub, ConnectionId, DbBridge, JournalWriter, WindowJournal, WorkspaceSessionStore,
};

use crate::multi_window::WindowOpenQueue;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WindowGeometry {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WindowPersistence {
    pub maximized: bool,
    pub geometry: Option<WindowGeometry>,
}

#[derive(Debug, Clone, Default)]
pub struct AppearanceConfig {
    pub theme_mode: crate::theme::ThemeMode,
    pub density: crate::theme::Density,
    pub ui_font: crate::theme::UiFontChoice,
    pub mono_font: crate::theme::MonoFontChoice,
}

#[derive(Debug, Clone)]
pub struct SettingsToast {
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Debug, Default, Clone)]
pub struct SettingsInbox {
    pub toasts: Vec<SettingsToast>,
    pub appearance: Option<AppearanceConfig>,
    /// Updated when the user resizes the row detail panel; merged into `settings.toml` on exit.
    pub row_detail_width: Option<f32>,
}

pub type SharedSettingsInbox = Arc<Mutex<SettingsInbox>>;

#[derive(Clone, Debug)]
pub struct PendingJournal {
    pub path: PathBuf,
    pub doc: WindowJournal,
}

pub type JournalShutdownRegistry = Arc<Mutex<Vec<JournalWriter>>>;

#[derive(Clone)]
pub struct LaunchConfig {
    pub window: WindowPersistence,
    pub appearance: AppearanceConfig,
    pub row_detail_width: f32,
    pub settings_inbox: Option<SharedSettingsInbox>,
    /// Tokio bridge for DB/network work (LUM-010).
    pub db_bridge: Option<Arc<DbBridge>>,
    /// Saved connection profiles (LUM-014 / LUM-015).
    pub connections: Option<Arc<ConnectionHub>>,
    /// Open tabs per connection (LUM-026).
    pub workspace_sessions: Arc<Mutex<WorkspaceSessionStore>>,
    pub window_open_queue: WindowOpenQueue,
    /// When set, this window opens directly into workspace for the connection.
    pub initial_connection: Option<ConnectionId>,
    /// Journals left from an unclean exit (LUM-035).
    pub pending_journals: Vec<PendingJournal>,
    /// Cleared on clean app exit so recovery files are removed.
    pub journal_shutdown: JournalShutdownRegistry,
}


#[derive(Debug, Clone, Default)]
pub struct ShellMetrics {
    pub first_frame_ms: Option<u128>,
}

#[derive(Debug, Clone)]
pub struct LaunchOutcome {
    pub window: WindowPersistence,
    pub appearance: AppearanceConfig,
    pub metrics: ShellMetrics,
}

