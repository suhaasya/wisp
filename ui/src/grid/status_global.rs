//! Shared grid status for the shell status bar.

use std::{cell::RefCell, rc::Rc};

use gpui::{App, Global};
use wisp_core::GridStatus;

#[derive(Clone, Default)]
pub struct GridStatusHandle(Rc<RefCell<Option<GridStatus>>>);

impl GridStatusHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, status: GridStatus) {
        *self.0.borrow_mut() = Some(status);
    }

    pub fn take_line(&self) -> Option<String> {
        self.0.borrow().as_ref().map(GridStatus::format_line)
    }
}

struct GridStatusGlobal(GridStatusHandle);

impl Global for GridStatusGlobal {}

pub fn init_grid_status(cx: &mut App) -> GridStatusHandle {
    let handle = GridStatusHandle::new();
    cx.set_global(GridStatusGlobal(handle.clone()));
    handle
}

pub fn grid_status(cx: &App) -> GridStatusHandle {
    cx.global::<GridStatusGlobal>().0.clone()
}
