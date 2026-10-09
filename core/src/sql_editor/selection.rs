//! Cursor and selection model for the SQL editor.

use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub fn caret(offset: usize) -> Self {
        Self {
            anchor: offset,
            head: offset,
        }
    }

    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    pub fn is_caret(&self) -> bool {
        self.anchor == self.head
    }

    pub fn head_min(&self) -> usize {
        self.anchor.min(self.head)
    }

    pub fn head_max(&self) -> usize {
        self.anchor.max(self.head)
    }
}
