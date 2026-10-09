//! GPUI-based widgets, layouts, and interaction for the Wisp client.
//!
//! Owns presentation and input handling wired to core session state. May depend on
//! [`wisp_core`] only among Wisp crates. Must not depend on drivers, transport, store,
//! or the application binary directly.
//!
//! GPUI is confined to this crate — downstream code uses [`prelude`] and [`run`].

mod app;
pub mod bridge;
pub mod connections;
mod environment;
mod journal;
mod launch;
mod memory;
mod multi_window;
pub mod prelude;
mod route;
mod shell;
pub mod components;
pub mod theme;
pub mod workspace;

pub mod grid;
pub mod render;
pub mod sql_editor;

pub use environment::Environment;
pub use launch::{
    AppearanceConfig, JournalShutdownRegistry, LaunchConfig, LaunchOutcome, PendingJournal,
    SettingsInbox, SettingsToast, SharedSettingsInbox, ShellMetrics, WindowGeometry,
    WindowPersistence,
};
pub use multi_window::WindowOpenQueue;
pub use route::Route;
pub use theme::{Density, MonoFontChoice, ThemeMode, UiFontChoice};
/// Placeholder until GPUI integration lands in LUM-005.
pub const CRATE_MARKER: &str = "wisp-ui";

/// Start the native GPUI application shell.
pub fn run(config: LaunchConfig) -> anyhow::Result<LaunchOutcome> {
    app::run(config)
}

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(super::CRATE_MARKER, "wisp-ui");
    }
}
