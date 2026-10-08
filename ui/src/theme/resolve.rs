//! Resolve tokens + typography + density into GPUI-facing values.

use gpui::Rgba;

use super::{
    contrast::env_label_foreground,
    density::Density,
    system::{detect_system_appearance, SystemAppearance},
    tokens::ColorTokens,
    typography::{Typography, MONO_FONT_SIZE_PX, UI_FONT_SIZE_PX},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Clone)]
pub struct ResolvedTheme {
    pub colors: ResolvedColors,
    pub density: Density,
    pub typography: Typography,
    pub ui_font_size: gpui::Pixels,
    pub mono_font_size: gpui::Pixels,
}

#[derive(Clone)]
pub struct ResolvedColors {
    pub canvas: Rgba,
    pub window: Rgba,
    pub sidebar: Rgba,
    pub panel: Rgba,
    pub raise: Rgba,
    pub ink1: Rgba,
    pub ink2: Rgba,
    pub ink3: Rgba,
    pub line: Rgba,
    pub line2: Rgba,
    pub accent: Rgba,
    pub accent_soft: Rgba,
    pub focus: Rgba,
    pub on_accent: Rgba,
    pub on_env: Rgba,
    pub env: EnvColors,
    pub staged: StagedEditColors,
    pub syntax: SyntaxColors,
}

#[derive(Clone, Copy)]
pub struct EnvColors {
    pub local: Rgba,
    pub dev: Rgba,
    pub staging: Rgba,
    pub production: Rgba,
    pub on_local: Rgba,
    pub on_dev: Rgba,
    pub on_staging: Rgba,
    pub on_production: Rgba,
}

#[derive(Clone, Copy)]
pub struct StagedEditColors {
    pub modified: Rgba,
    pub modified_line: Rgba,
    pub inserted: Rgba,
    pub deleted: Rgba,
}

#[derive(Clone, Copy)]
pub struct SyntaxColors {
    pub keyword: Rgba,
    pub string: Rgba,
    pub number: Rgba,
    pub function: Rgba,
    pub comment: Rgba,
    pub null: Rgba,
}

impl ResolvedTheme {
    pub fn resolve(mode: ThemeMode, density: Density, typography: Typography) -> Self {
        let appearance = match mode {
            ThemeMode::Light => SystemAppearance::Light,
            ThemeMode::Dark => SystemAppearance::Dark,
            ThemeMode::System => detect_system_appearance(),
        };
        let tokens = match appearance {
            SystemAppearance::Light => ColorTokens::LIGHT,
            SystemAppearance::Dark => ColorTokens::DARK,
        };
        Self {
            colors: ResolvedColors::from_tokens(tokens),
            density,
            typography,
            ui_font_size: gpui::px(UI_FONT_SIZE_PX),
            mono_font_size: gpui::px(MONO_FONT_SIZE_PX),
        }
    }
}

impl ResolvedColors {
    fn from_tokens(tokens: ColorTokens) -> Self {
        let s = tokens.surfaces;
        Self {
            canvas: hex(s.canvas),
            window: hex(s.window),
            sidebar: hex(s.sidebar),
            panel: hex(s.panel),
            raise: hex(s.raise),
            ink1: hex(s.ink1),
            ink2: hex(s.ink2),
            ink3: hex(s.ink3),
            line: hex(s.line),
            line2: hex(s.line2),
            accent: hex(s.accent),
            accent_soft: hex(s.accent_soft),
            focus: hex(s.focus),
            on_accent: hex(s.on_accent),
            on_env: hex(s.on_env),
            env: EnvColors {
                local: hex(tokens.env.local),
                dev: hex(tokens.env.dev),
                staging: hex(tokens.env.staging),
                production: hex(tokens.env.production),
                on_local: hex(env_label_foreground(
                    tokens.env.local,
                    super::contrast::ENV_LABEL_DARK,
                    super::contrast::ENV_LABEL_LIGHT,
                )),
                on_dev: hex(env_label_foreground(
                    tokens.env.dev,
                    super::contrast::ENV_LABEL_DARK,
                    super::contrast::ENV_LABEL_LIGHT,
                )),
                on_staging: hex(env_label_foreground(
                    tokens.env.staging,
                    super::contrast::ENV_LABEL_DARK,
                    super::contrast::ENV_LABEL_LIGHT,
                )),
                on_production: hex(env_label_foreground(
                    tokens.env.production,
                    super::contrast::ENV_LABEL_DARK,
                    super::contrast::ENV_LABEL_LIGHT,
                )),
            },
            staged: StagedEditColors {
                modified: hex(tokens.staged.modified),
                modified_line: hex(tokens.staged.modified_line),
                inserted: hex(tokens.staged.inserted),
                deleted: hex(tokens.staged.deleted),
            },
            syntax: SyntaxColors {
                keyword: hex(tokens.syntax.keyword),
                string: hex(tokens.syntax.string),
                number: hex(tokens.syntax.number),
                function: hex(tokens.syntax.function),
                comment: hex(tokens.syntax.comment),
                null: hex(tokens.syntax.null),
            },
        }
    }
}

pub fn hex(value: u32) -> Rgba {
    gpui::rgb(value)
}
