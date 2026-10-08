//! Design tokens from `design/lumen-design-mockup.html`.

use gpui::Rgba;

#[derive(Clone)]
pub struct Theme {
    pub canvas: Rgba,
    pub window: Rgba,
    pub side: Rgba,
    pub panel: Rgba,
    pub ink: Rgba,
    pub ink2: Rgba,
    pub ink3: Rgba,
    pub line: Rgba,
    pub accent: Rgba,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            canvas: rgb(0xE9ECF1),
            window: rgb(0xFBFCFD),
            side: rgb(0xF1F3F7),
            panel: rgb(0xFFFFFF),
            ink: rgb(0x1D222B),
            ink2: rgb(0x5A6372),
            ink3: rgb(0x8A93A3),
            line: rgb(0xDDE2E9),
            accent: rgb(0x0E8A86),
        }
    }
}

pub fn rgb(hex: u32) -> Rgba {
    gpui::rgb(hex)
}
