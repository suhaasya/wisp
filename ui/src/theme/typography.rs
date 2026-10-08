//! Typography tokens (sizes + font family selection).

/// UI body size from the mockup (`13px / 1.45`).
pub const UI_FONT_SIZE_PX: f32 = 13.0;
/// Mono size for cells and editor (`12.5px` in mockup).
pub const MONO_FONT_SIZE_PX: f32 = 12.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFontChoice {
    /// System UI font (`.SystemUIFont` in GPUI — no bundled file).
    #[default]
    System,
    /// Reserved for a single bundled UI font in a later milestone.
    IbmPlexSans,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonoFontChoice {
    #[default]
    System,
    IbmPlexMono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Typography {
    pub ui_font: UiFontChoice,
    pub mono_font: MonoFontChoice,
}

impl Typography {
    pub fn gpui_ui_font(&self) -> gpui::Font {
        match self.ui_font {
            UiFontChoice::System => gpui::font(".SystemUIFont"),
            UiFontChoice::IbmPlexSans => gpui::font("IBM Plex Sans"),
        }
    }

    pub fn gpui_mono_font(&self) -> gpui::Font {
        match self.mono_font {
            MonoFontChoice::System => gpui::font("Menlo"),
            MonoFontChoice::IbmPlexMono => gpui::font("IBM Plex Mono"),
        }
    }
}
