//! Public UI surface for non-`wisp-ui` crates. Does **not** re-export GPUI types.

pub use crate::{
    run, AppearanceConfig, Density, Environment, LaunchConfig, LaunchOutcome, MonoFontChoice,
    Route, ShellMetrics, ThemeMode, UiFontChoice, CRATE_MARKER,
};
