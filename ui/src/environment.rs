//! Connection environment indicator (title strip + status dot).

use gpui::Rgba;

use crate::theme::rgb;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Environment {
    #[default]
    Local,
    Dev,
    Staging,
    Production,
}

impl Environment {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::Dev => "Dev",
            Self::Staging => "Staging",
            Self::Production => "Prod",
        }
    }

    pub fn color(self) -> Rgba {
        match self {
            Self::Local => rgb(0x3A9D5D),
            Self::Dev => rgb(0x3B7DD8),
            Self::Staging => rgb(0xC98516),
            Self::Production => rgb(0xCF4136),
        }
    }
}
