//! Grid UI preferences (panel width, etc.) synced to `settings.toml` on exit.

use gpui::{App, Global, UpdateGlobal};

use crate::launch::SharedSettingsInbox;

#[derive(Clone)]
pub struct GridSettingsGlobal {
    pub row_detail_width: f32,
    settings_inbox: Option<SharedSettingsInbox>,
}

impl Global for GridSettingsGlobal {}

pub fn init(cx: &mut App, row_detail_width: f32, settings_inbox: Option<SharedSettingsInbox>) {
    cx.set_global(GridSettingsGlobal {
        row_detail_width,
        settings_inbox,
    });
}

pub fn row_detail_width(cx: &App) -> f32 {
    cx.global::<GridSettingsGlobal>().row_detail_width
}

pub fn set_row_detail_width<C: gpui::BorrowAppContext>(cx: &mut C, width: f32) {
    GridSettingsGlobal::update_global(cx, |g, _| {
        g.row_detail_width = width;
        if let Some(inbox) = g.settings_inbox.as_ref() {
            if let Ok(mut guard) = inbox.lock() {
                guard.row_detail_width = Some(width);
            }
        }
    });
}
