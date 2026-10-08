//! Layout density (row heights).

use gpui::Pixels;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    /// 26 px rows (mockup status bar height).
    #[default]
    Compact,
    /// 32 px rows for touch / accessibility.
    Comfortable,
}

impl Density {
    pub fn row_height_px(self) -> f32 {
        match self {
            Self::Compact => 26.0,
            Self::Comfortable => 32.0,
        }
    }

    pub fn row_height(self) -> Pixels {
        gpui::px(self.row_height_px())
    }

    pub fn status_bar_height(self) -> Pixels {
        self.row_height()
    }
}
