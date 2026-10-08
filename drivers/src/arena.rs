//! Single bump buffer per result page (strings and byte prefixes).

use std::str;

/// Byte range into a page [`PageArena`](super::page::PageArena) buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlobRef {
    pub offset: u32,
    pub len: u32,
}

impl BlobRef {
    pub fn slice<'a>(&self, arena: &'a [u8]) -> &'a [u8] {
        let start = self.offset as usize;
        let end = start + self.len as usize;
        &arena[start..end]
    }

    pub fn as_str<'a>(&self, arena: &'a [u8]) -> Option<&'a str> {
        str::from_utf8(self.slice(arena)).ok()
    }
}

pub type StrRef = BlobRef;
