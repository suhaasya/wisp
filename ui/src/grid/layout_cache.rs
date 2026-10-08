//! Cached formatted cell strings for visible coordinates.

use std::collections::HashMap;

use gpui::SharedString;

const MAX_ENTRIES: usize = 4_096;

#[derive(Debug, Default)]
pub struct LayoutCache {
    entries: HashMap<(u64, u16), SharedString>,
}

impl LayoutCache {
    pub fn get(&self, row: u64, col: u16) -> Option<SharedString> {
        self.entries.get(&(row, col)).cloned()
    }

    pub fn put(&mut self, row: u64, col: u16, text: impl Into<SharedString>) {
        if self.entries.len() >= MAX_ENTRIES {
            self.entries.clear();
        }
        self.entries.insert((row, col), text.into());
    }
}
