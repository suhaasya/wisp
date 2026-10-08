//! Single-line text editing logic (GPUI-agnostic; unit tested).

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

const MAX_UNDO: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    content: String,
    selection: Range<usize>,
    selection_reversed: bool,
}

/// Editable single-line buffer with selection and undo/redo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SingleLineEditor {
    content: String,
    selection: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl SingleLineEditor {
    pub fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        let end = content.len();
        Self {
            content,
            selection: 0..end,
            ..Default::default()
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }

    pub fn selection_reversed(&self) -> bool {
        self.selection_reversed
    }

    pub fn marked_range(&self) -> Option<Range<usize>> {
        self.marked_range.clone()
    }

    pub fn set_marked_range(&mut self, range: Option<Range<usize>>) {
        self.marked_range = range;
    }

    pub fn clear_marked_range(&mut self) {
        self.marked_range = None;
    }

    pub fn cursor(&self) -> usize {
        if self.selection_reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    pub fn move_to(&mut self, offset: usize) {
        let offset = offset.clamp(0, self.content.len());
        self.selection = offset..offset;
        self.selection_reversed = false;
    }

    pub fn select_to(&mut self, offset: usize) {
        let offset = offset.clamp(0, self.content.len());
        if self.selection_reversed {
            self.selection.start = offset;
        } else {
            self.selection.end = offset;
        }
        if self.selection.end < self.selection.start {
            self.selection_reversed = !self.selection_reversed;
            self.selection = self.selection.end..self.selection.start;
        }
    }

    pub fn select_all(&mut self) {
        self.selection_reversed = false;
        self.selection = 0..self.content.len();
    }

    pub fn insert(&mut self, text: &str) {
        let sanitized = text.replace('\n', " ");
        if sanitized.is_empty() {
            return;
        }
        self.replace_range(self.effective_replace_range(), &sanitized, true);
    }

    pub fn delete_backward(&mut self) {
        if self.selection.is_empty() {
            let prev = self.previous_boundary(self.cursor());
            if prev == self.cursor() {
                return;
            }
            self.select_to(prev);
        }
        self.replace_range(self.selection.clone(), "", true);
    }

    pub fn delete_forward(&mut self) {
        if self.selection.is_empty() {
            let next = self.next_boundary(self.cursor());
            if next == self.cursor() {
                return;
            }
            self.select_to(next);
        }
        self.replace_range(self.selection.clone(), "", true);
    }

    pub fn replace_range(&mut self, range: Range<usize>, text: &str, record_undo: bool) {
        let range = self.clamp_range(range);
        if record_undo {
            self.push_undo();
        }
        self.content.replace_range(range.clone(), text);
        let cursor = range.start + text.len();
        self.selection = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        self.redo.clear();
    }

    pub fn replace_for_ime(
        &mut self,
        range: Range<usize>,
        text: &str,
        new_selection: Option<Range<usize>>,
    ) {
        self.push_undo();
        let range = self.clamp_range(range);
        self.content.replace_range(range.clone(), text);
        if !text.is_empty() {
            self.marked_range = Some(range.start..range.start + text.len());
        } else {
            self.marked_range = None;
        }
        self.selection = new_selection.map_or_else(
            || {
                let c = range.start + text.len();
                c..c
            },
            |r| range.start + r.start..range.start + r.end,
        );
        self.selection_reversed = false;
        self.redo.clear();
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        self.redo.push(Snapshot {
            content: self.content.clone(),
            selection: self.selection.clone(),
            selection_reversed: self.selection_reversed,
        });
        self.content = snapshot.content;
        self.selection = snapshot.selection;
        self.selection_reversed = snapshot.selection_reversed;
        self.marked_range = None;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        self.undo.push(Snapshot {
            content: self.content.clone(),
            selection: self.selection.clone(),
            selection_reversed: self.selection_reversed,
        });
        self.content = snapshot.content;
        self.selection = snapshot.selection;
        self.selection_reversed = snapshot.selection_reversed;
        self.marked_range = None;
        true
    }

    pub fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    pub fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    pub fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn effective_replace_range(&self) -> Range<usize> {
        self.marked_range
            .clone()
            .unwrap_or_else(|| self.selection.clone())
    }

    fn push_undo(&mut self) {
        if self.undo.len() >= MAX_UNDO {
            self.undo.remove(0);
        }
        self.undo.push(Snapshot {
            content: self.content.clone(),
            selection: self.selection.clone(),
            selection_reversed: self.selection_reversed,
        });
    }

    fn clamp_range(&self, range: Range<usize>) -> Range<usize> {
        let start = range.start.min(self.content.len());
        let end = range.end.min(self.content.len());
        start..end.max(start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_delete() {
        let mut ed = SingleLineEditor::default();
        ed.insert("hello");
        assert_eq!(ed.content(), "hello");
        ed.move_to(5);
        ed.insert("!");
        assert_eq!(ed.content(), "hello!");
        ed.delete_backward();
        assert_eq!(ed.content(), "hello");
        ed.move_to(0);
        ed.delete_forward();
        assert_eq!(ed.content(), "ello");
    }

    #[test]
    fn selection_replace() {
        let mut ed = SingleLineEditor::new("abcd");
        ed.select_all();
        ed.replace_range(ed.selection(), "xy", true);
        assert_eq!(ed.content(), "xy");
    }

    #[test]
    fn undo_redo() {
        let mut ed = SingleLineEditor::default();
        ed.insert("a");
        ed.insert("b");
        assert_eq!(ed.content(), "ab");
        assert!(ed.undo());
        assert_eq!(ed.content(), "a");
        assert!(ed.redo());
        assert_eq!(ed.content(), "ab");
    }

    #[test]
    fn newline_becomes_space() {
        let mut ed = SingleLineEditor::default();
        ed.insert("a\nb");
        assert_eq!(ed.content(), "a b");
    }
}
