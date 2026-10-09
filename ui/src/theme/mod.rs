//! Design tokens, palettes, and the global theme (LUM-006).

mod contrast;
mod density;
mod global;
mod resolve;
mod system;
mod tokens;

pub use resolve::SyntaxColors;
mod typography;

pub use contrast::{evaluate_pair, wcag_pairs, ContrastPair, ContrastResult};
pub use density::Density;
pub use global::{init as init_global, read as read_global, update as update_global, ThemeGlobal};
pub use resolve::{EnvColors, ResolvedTheme, ThemeMode};
pub use typography::{
    MonoFontChoice, Typography, UiFontChoice, MONO_FONT_SIZE_PX, UI_FONT_SIZE_PX,
};

pub use resolve::ResolvedColors;
