//! Window launch/persistence DTOs (mapped from `wisp-store` in the binary crate).

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

#[derive(Debug, Clone, Default)]
pub struct LaunchConfig {
    pub window: WindowPersistence,
    pub appearance: AppearanceConfig,
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
