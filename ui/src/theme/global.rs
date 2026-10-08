//! Process-wide theme state (GPUI global).

use std::borrow::{Borrow, BorrowMut};

use gpui::{App, Global, ReadGlobal, UpdateGlobal};

use super::{
    density::Density,
    resolve::{ResolvedTheme, ThemeMode},
    system::{detect_system_appearance, SystemAppearance},
    typography::{MonoFontChoice, Typography, UiFontChoice},
};

/// GPUI global holding theme preferences and the resolved palette.
#[derive(Clone)]
pub struct ThemeGlobal {
    pub mode: ThemeMode,
    pub density: Density,
    pub typography: Typography,
    resolved: ResolvedTheme,
}

impl ThemeGlobal {
    pub fn new(mode: ThemeMode, density: Density, typography: Typography) -> Self {
        let resolved = ResolvedTheme::resolve(mode, density, typography);
        Self {
            mode,
            density,
            typography,
            resolved,
        }
    }

    pub fn resolved(&self) -> &ResolvedTheme {
        &self.resolved
    }

    pub fn set_mode<C: gpui::BorrowAppContext + BorrowMut<App>>(
        &mut self,
        mode: ThemeMode,
        cx: &mut C,
    ) {
        self.mode = mode;
        self.recompute();
        cx.borrow_mut().refresh_windows();
    }

    pub fn toggle_light_dark<C: gpui::BorrowAppContext + BorrowMut<App>>(&mut self, cx: &mut C) {
        let next = match self.effective_appearance() {
            SystemAppearance::Light => ThemeMode::Dark,
            SystemAppearance::Dark => ThemeMode::Light,
        };
        self.set_mode(next, cx);
    }

    pub fn set_density<C: gpui::BorrowAppContext + BorrowMut<App>>(
        &mut self,
        density: Density,
        cx: &mut C,
    ) {
        self.density = density;
        self.recompute();
        cx.borrow_mut().refresh_windows();
    }

    pub fn apply_appearance<C: gpui::BorrowAppContext + BorrowMut<App>>(
        &mut self,
        mode: ThemeMode,
        density: Density,
        typography: Typography,
        cx: &mut C,
    ) {
        self.mode = mode;
        self.density = density;
        self.typography = typography;
        self.recompute();
        cx.borrow_mut().refresh_windows();
    }

    pub fn effective_appearance(&self) -> SystemAppearance {
        match self.mode {
            ThemeMode::Light => SystemAppearance::Light,
            ThemeMode::Dark => SystemAppearance::Dark,
            ThemeMode::System => detect_system_appearance(),
        }
    }

    fn recompute(&mut self) {
        self.resolved = ResolvedTheme::resolve(self.mode, self.density, self.typography);
    }

    pub fn sync_system<C: gpui::BorrowAppContext + BorrowMut<App>>(&mut self, cx: &mut C) {
        if self.mode == ThemeMode::System {
            self.recompute();
            cx.borrow_mut().refresh_windows();
        }
    }
}

impl Global for ThemeGlobal {}

impl Default for ThemeGlobal {
    fn default() -> Self {
        Self::new(ThemeMode::System, Density::default(), Typography::default())
    }
}

/// Install defaults before opening windows.
pub fn init<C: gpui::BorrowAppContext>(
    cx: &mut C,
    mode: ThemeMode,
    density: Density,
    ui_font: UiFontChoice,
    mono_font: MonoFontChoice,
) {
    ThemeGlobal::set_global(
        cx,
        ThemeGlobal::new(mode, density, Typography { ui_font, mono_font }),
    );
}

pub fn read<C: Borrow<App>>(cx: &C) -> &ThemeGlobal {
    ThemeGlobal::global(cx.borrow())
}

pub fn update<C, F, R>(cx: &mut C, f: F) -> R
where
    C: gpui::BorrowAppContext,
    F: FnOnce(&mut ThemeGlobal, &mut C) -> R,
{
    ThemeGlobal::update_global(cx, f)
}
