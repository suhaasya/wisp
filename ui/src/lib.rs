//! GPUI-based widgets, layouts, and interaction for the Wisp client.
//!
//! Owns presentation and input handling wired to core session state. May depend on
//! [`wisp_core`] only among Wisp crates. Must not depend on drivers, transport, store,
//! or the application binary directly.
//!
//! GPUI is confined to this crate — downstream code uses [`prelude`] and [`run`].

mod app;
mod environment;
mod launch;
mod memory;
pub mod prelude;
mod route;
mod shell;
pub mod components;
pub mod theme;

pub mod grid;
pub mod render;

pub use environment::Environment;
pub use launch::{
    AppearanceConfig, LaunchConfig, LaunchOutcome, ShellMetrics, WindowGeometry, WindowPersistence,
};
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
