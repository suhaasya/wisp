//! Rope-backed SQL editor core (LUM-030).

use std::ops::Range;
use std::time::{Duration, Instant};

use ropey::Rope;
use unicode_segmentation::UnicodeSegmentation;

use super::selection::Selection;
use super::statement_split::{current_statement_range, SplitFlavor};

const MAX_UNDO: usize = 256;
const BURST_GAP: Duration = Duration::from_millis(400);

#[derive(Debug, Clone)]
struct HistoryEntry {
    text: Rope,
    selections: Vec<Selection>,
}

#[derive(Debug, Clone)]
pub struct SqlEditor {
    rope: Rope,
    selections: Vec<Selection>,
    soft_wrap: bool,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    last_edit: Option<Instant>,
}

impl Default for SqlEditor {
    fn default() -> Self {
        Self::new("")
    }
}

impl SqlEditor {
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            selections: vec![Selection::caret(0)],
            soft_wrap: false,
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
        }
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Replace buffer contents (used when restoring crash journal).
    pub fn set_text(&mut self, text: &str) {
        self.rope = Rope::from_str(text);
        self.selections = vec![Selection::caret(0)];
        self.undo.clear();
        self.redo.clear();
        self.last_edit = None;
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn soft_wrap(&self) -> bool {
        self.soft_wrap
    }

    pub fn set_soft_wrap(&mut self, on: bool) {
        self.soft_wrap = on;
    }

    pub fn selections(&self) -> &[Selection] {
        &self.selections
    }

    pub fn primary_selection(&self) -> Selection {
        self.selections[0]
    }

    pub fn set_primary_caret(&mut self, offset: usize) {
        let o = offset.min(self.rope.len_chars());
        self.selections = vec![Selection::caret(o)];
    }

    pub fn set_primary_selection(&mut self, selection: Selection) {
        self.selections = vec![selection];
    }

    pub fn select_all(&mut self) {
        let end = self.rope.len_chars();
        self.selections = vec![Selection { anchor: 0, head: end }];
    }

    pub fn add_cursor_at(&mut self, offset: usize) {
        let o = offset.min(self.rope.len_chars());
        if !self.selections.iter().any(|s| s.head == o && s.is_caret()) {
            self.selections.push(Selection::caret(o));
        }
    }

    fn begin_edit(&mut self) {
        let now = Instant::now();
        let burst = self
            .last_edit
            .is_some_and(|t| now.duration_since(t) < BURST_GAP);
        if !burst {
            let entry = HistoryEntry {
                text: self.rope.clone(),
                selections: self.selections.clone(),
            };
            self.undo.push(entry);
            if self.undo.len() > MAX_UNDO {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        self.last_edit = Some(now);
    }

    pub fn insert(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.begin_edit();
        let mut sorted: Vec<usize> = self.selections.iter().map(|s| s.head).collect();
        sorted.sort_unstable_by(|a, b| b.cmp(a));
        for head in sorted {
            self.rope.insert(head, text);
            self.update_selections_after_insert(head, text.chars().count());
        }
    }

    pub fn replace_selections_with(&mut self, text: &str) {
        self.begin_edit();
        let mut specs: Vec<(Range<usize>, usize)> = self
            .selections
            .iter()
            .map(|s| (s.range(), s.head))
            .collect();
        specs.sort_by(|a, b| b.0.start.cmp(&a.0.start));
        let mut new_selections = Vec::new();
        for (range, _) in specs {
            self.rope.remove(range.clone());
            self.rope.insert(range.start, text);
            let end = range.start + text.chars().count();
            new_selections.push(Selection::caret(end));
        }
        if new_selections.is_empty() {
            new_selections.push(Selection::caret(0));
        }
        self.selections = new_selections;
    }

    pub fn delete_backward(&mut self) {
        self.begin_edit();
        let mut heads: Vec<usize> = self.selections.iter().map(|s| s.head).collect();
        heads.sort_unstable_by(|a, b| b.cmp(a));
        for head in heads {
            if head == 0 {
                continue;
            }
            let prev = self.prev_boundary(head);
            self.rope.remove(prev..head);
            self.update_selections_after_delete(prev, head - prev);
        }
    }

    pub fn delete_forward(&mut self) {
        self.begin_edit();
        let mut heads: Vec<usize> = self.selections.iter().map(|s| s.head).collect();
        heads.sort_unstable_by(|a, b| b.cmp(a));
        for head in heads {
            if head >= self.rope.len_chars() {
                continue;
            }
            let next = self.next_boundary(head);
            self.rope.remove(head..next);
            self.update_selections_after_delete(head, next - head);
        }
    }

    pub fn undo(&mut self) {
        let Some(entry) = self.undo.pop() else {
            return;
        };
        self.redo.push(HistoryEntry {
            text: self.rope.clone(),
            selections: self.selections.clone(),
        });
        self.rope = entry.text;
        self.selections = entry.selections;
    }

    pub fn redo(&mut self) {
        let Some(entry) = self.redo.pop() else {
            return;
        };
        self.undo.push(HistoryEntry {
            text: self.rope.clone(),
            selections: self.selections.clone(),
        });
        self.rope = entry.text;
        self.selections = entry.selections;
    }

    pub fn move_left(&mut self, extend: bool) {
        for sel in &mut self.selections {
            let next = sel.head.saturating_sub(1);
            if extend {
                sel.head = next;
            } else {
                *sel = Selection::caret(next);
            }
        }
    }

    pub fn move_right(&mut self, extend: bool) {
        let max = self.rope.len_chars();
        for sel in &mut self.selections {
            let next = (sel.head + 1).min(max);
            if extend {
                sel.head = next;
            } else {
                *sel = Selection::caret(next);
            }
        }
    }

    pub fn move_word_left(&mut self) {
        let heads: Vec<usize> = self
            .selections
            .iter()
            .map(|s| self.prev_word_boundary(s.head))
            .collect();
        for (sel, head) in self.selections.iter_mut().zip(heads) {
            sel.head = head;
            sel.anchor = head;
        }
    }

    pub fn move_word_right(&mut self) {
        let heads: Vec<usize> = self
            .selections
            .iter()
            .map(|s| self.next_word_boundary(s.head))
            .collect();
        for (sel, head) in self.selections.iter_mut().zip(heads) {
            sel.head = head;
            sel.anchor = head;
        }
    }

    pub fn move_line_start(&mut self) {
        for sel in &mut self.selections {
            let line = self.rope.char_to_line(sel.head);
            sel.head = self.rope.line_to_char(line);
            sel.anchor = sel.head;
        }
    }

    pub fn move_line_end(&mut self) {
        for sel in &mut self.selections {
            let line = self.rope.char_to_line(sel.head);
            let start = self.rope.line_to_char(line);
            let next_line = line + 1;
            sel.head = if next_line < self.rope.len_lines() {
                self.rope.line_to_char(next_line).saturating_sub(1)
            } else {
                self.rope.len_chars()
            };
            sel.anchor = sel.head;
            let _ = start;
        }
    }

    pub fn matching_bracket(&self, head: usize) -> Option<usize> {
        let ch = self.rope.get_char(head)?;
        let (open, close, dir) = match ch {
            '(' => ('(', ')', 1),
            ')' => (')', '(', -1),
            '[' => ('[', ']', 1),
            ']' => (']', '[', -1),
            '{' => ('{', '}', 1),
            '}' => ('}', '{', -1),
            _ => return None,
        };
        let mut depth = 0;
        let mut i = head;
        if dir > 0 {
            while i < self.rope.len_chars() {
                if self.rope.get_char(i) == Some(open) {
                    depth += 1;
                } else if self.rope.get_char(i) == Some(close) {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                i += 1;
            }
        } else {
            while i > 0 {
                if self.rope.get_char(i) == Some(close) {
                    depth += 1;
                } else if self.rope.get_char(i) == Some(open) {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                i -= 1;
            }
        }
        None
    }

    pub fn find_next(&self, needle: &str, from: usize, case_sensitive: bool) -> Option<Range<usize>> {
        if needle.is_empty() {
            return None;
        }
        let hay = self.rope.to_string();
        let start = from.min(hay.len());
        let hay_search = if case_sensitive {
            hay[start..].to_string()
        } else {
            hay[start..].to_lowercase()
        };
        let needle_search = if case_sensitive {
            needle.to_string()
        } else {
            needle.to_lowercase()
        };
        hay_search.find(&needle_search).map(|ix| {
            let byte_start = start + ix;
            let char_start = hay[..byte_start].chars().count();
            char_start..char_start + needle.chars().count()
        })
    }

    pub fn replace_byte_range(&mut self, range: Range<usize>, text: &str) {
        let start = self.rope.byte_to_char(range.start.min(self.rope.len_bytes()));
        let end = self
            .rope
            .byte_to_char(range.end.min(self.rope.len_bytes()));
        self.replace_primary(start..end, text);
    }

    pub fn replace_primary(&mut self, range: Range<usize>, text: &str) {
        if range.is_empty() && text.is_empty() {
            return;
        }
        self.begin_edit();
        if !range.is_empty() {
            self.rope.remove(range.clone());
        }
        self.rope.insert(range.start, text);
        let end = range.start + text.chars().count();
        self.selections = vec![Selection::caret(end)];
    }

    pub fn replace_range(&mut self, range: Range<usize>, text: &str) {
        self.begin_edit();
        self.rope.remove(range.clone());
        self.rope.insert(range.start, text);
        self.selections = vec![Selection::caret(range.start + text.chars().count())];
    }

    pub fn replace_all(&mut self, needle: &str, replacement: &str, case_sensitive: bool) {
        self.begin_edit();
        let mut offset = 0usize;
        while let Some(range) = self.find_next(needle, offset, case_sensitive) {
            self.rope.remove(range.clone());
            self.rope.insert(range.start, replacement);
            offset = range.start + replacement.chars().count();
        }
    }

    /// Byte range in UTF-8 for the statement under the primary caret.
    pub fn current_statement_bytes(&self, flavor: SplitFlavor) -> Range<usize> {
        let text = self.rope.to_string();
        let head = self.primary_selection().head.min(self.rope.len_chars());
        let byte = self.rope.char_to_byte(head).min(text.len());
        current_statement_range(&text, byte, flavor)
    }

    pub fn char_to_line(&self, char_ix: usize) -> usize {
        self.rope.char_to_line(char_ix.min(self.rope.len_chars()))
    }

    pub fn line_start_char(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return self.rope.len_chars();
        }
        self.rope.line_to_char(line)
    }

    pub fn caret_byte(&self) -> usize {
        let head = self.primary_selection().head.min(self.rope.len_chars());
        self.rope.char_to_byte(head)
    }

    pub fn primary_selection_bytes(&self) -> Range<usize> {
        let sel = self.primary_selection();
        let start = self.rope.char_to_byte(sel.head_min());
        let end = self.rope.char_to_byte(sel.head_max());
        start..end
    }

    pub fn line_start_byte(&self, line: usize) -> usize {
        let char_ix = self.line_start_char(line);
        self.rope.char_to_byte(char_ix)
    }

    pub fn byte_to_line(&self, byte: usize) -> usize {
        let byte = byte.min(self.rope.len_bytes());
        let ch = self.rope.byte_to_char(byte);
        self.rope.char_to_line(ch)
    }

    pub fn line_text(&self, line: usize) -> String {
        if line >= self.rope.len_lines() {
            return String::new();
        }
        self.rope.line(line).to_string()
    }

    fn prev_boundary(&self, head: usize) -> usize {
        head.saturating_sub(1)
    }

    fn next_boundary(&self, head: usize) -> usize {
        (head + 1).min(self.rope.len_chars())
    }

    fn prev_word_boundary(&self, head: usize) -> usize {
        if head == 0 {
            return 0;
        }
        let s: String = self.rope.to_string();
        let before: String = s.chars().take(head).collect();
        let mut idx = before.len();
        for (i, _) in before.grapheme_indices(true).rev() {
            let ch = before[i..].chars().next().unwrap_or(' ');
            if is_word_char(ch) {
                idx = i;
                break;
            }
        }
        before[..idx].chars().count()
    }

    fn next_word_boundary(&self, head: usize) -> usize {
        let max = self.rope.len_chars();
        if head >= max {
            return max;
        }
        let s = self.rope.to_string();
        let after: String = s.chars().skip(head).collect();
        let mut skip = 0usize;
        for (i, ch) in after.grapheme_indices(true) {
            if is_word_char(after[i..].chars().next().unwrap_or(' ')) {
                skip = i;
                break;
            }
        }
        for (i, ch) in after[skip..].grapheme_indices(true) {
            if !is_word_char(after[skip + i..].chars().next().unwrap_or(' ')) {
                return head + (skip + i).min(after.len());
            }
        }
        max
    }

    fn update_selections_after_insert(&mut self, at: usize, len: usize) {
        for sel in &mut self.selections {
            if sel.head >= at {
                sel.head += len;
            }
            if sel.anchor >= at {
                sel.anchor += len;
            }
        }
    }

    fn update_selections_after_delete(&mut self, at: usize, len: usize) {
        for sel in &mut self.selections {
            if sel.head > at {
                sel.head = sel.head.saturating_sub(len);
            }
            if sel.anchor > at {
                sel.anchor = sel.anchor.saturating_sub(len);
            }
        }
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_restores_cursors() {
        let mut ed = SqlEditor::new("hello");
        ed.set_primary_caret(5);
        ed.insert("!");
        assert_eq!(ed.primary_selection().head, 6);
        ed.undo();
        assert_eq!(ed.text(), "hello");
        assert_eq!(ed.primary_selection().head, 5);
    }

    #[test]
    fn multi_cursor_insert() {
        let mut ed = SqlEditor::new("ab");
        ed.selections = vec![Selection::caret(0), Selection::caret(2)];
        ed.insert("X");
        assert_eq!(ed.text(), "XabX");
    }

    #[test]
    fn insert_latency_10k_lines() {
        let text: String = (0..10_000)
            .map(|i| format!("SELECT {i};\n"))
            .collect();
        let mut ed = SqlEditor::new(&text);
        ed.set_primary_caret(ed.len_chars());
        let start = Instant::now();
        ed.insert("x");
        let elapsed = start.elapsed();
        let budget = if cfg!(debug_assertions) {
            Duration::from_millis(80)
        } else {
            Duration::from_millis(8)
        };
        assert!(
            elapsed < budget,
            "insert took {:?}, budget {:?}",
            elapsed,
            budget
        );
    }

    #[test]
    fn typing_burst_groups_undo() {
        let mut ed = SqlEditor::new("");
        ed.insert("a");
        ed.insert("b");
        ed.undo();
        assert_eq!(ed.text(), "");
    }
}
