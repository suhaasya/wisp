//! UTF-16 index helpers for GPUI input (char indices match [`wisp_core::SqlEditor`]).

use std::ops::Range;

pub fn char_index_from_utf16(text: &str, offset: usize) -> usize {
    let mut utf16_count = 0;
    for (char_index, ch) in text.chars().enumerate() {
        if utf16_count >= offset {
            return char_index;
        }
        utf16_count += ch.len_utf16();
    }
    text.chars().count()
}

pub fn char_index_to_utf16(text: &str, char_index: usize) -> usize {
    text.chars()
        .take(char_index)
        .map(|c| c.len_utf16())
        .sum()
}

pub fn range_from_utf16(text: &str, range_utf16: &Range<usize>) -> Range<usize> {
    char_index_from_utf16(text, range_utf16.start)
        ..char_index_from_utf16(text, range_utf16.end)
}

pub fn range_to_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    char_index_to_utf16(text, range.start)..char_index_to_utf16(text, range.end)
}
