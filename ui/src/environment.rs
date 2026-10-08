//! Connection environment indicator (title strip + status dot).

use gpui::Rgba;
use wisp_core::EnvironmentTag;

use crate::theme::EnvColors;

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

    pub fn color(self, env: &EnvColors) -> Rgba {
        match self {
            Self::Local => env.local,
            Self::Dev => env.dev,
            Self::Staging => env.staging,
            Self::Production => env.production,
        }
    }

    pub fn on_color(self, env: &EnvColors) -> Rgba {
        match self {
            Self::Local => env.on_local,
            Self::Dev => env.on_dev,
            Self::Staging => env.on_staging,
            Self::Production => env.on_production,
        }
    }

    pub fn from_tag(tag: &EnvironmentTag) -> Self {
        match tag {
            EnvironmentTag::Production => Self::Production,
            EnvironmentTag::Staging => Self::Staging,
            EnvironmentTag::Development => Self::Dev,
            EnvironmentTag::Custom(label) if label.eq_ignore_ascii_case("local") => Self::Local,
            EnvironmentTag::Custom(_) => Self::Dev,
        }
    }

    pub fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }
}
