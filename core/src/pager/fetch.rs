//! Page fetch requests issued by the pager.

use super::strategy::KeysetCursor;

#[derive(Debug, Clone, PartialEq)]
pub enum PageFetch {
    Offset { offset: u64, limit: u32 },
    KeysetAfter {
        cursor: KeysetCursor,
        limit: u32,
    },
    /// Server-held cursor / portal (implementation in drivers, LUM-024+).
    ServerCursor { cursor_id: u64, limit: u32 },
}

impl PageFetch {
    pub fn limit(&self) -> u32 {
        match self {
            Self::Offset { limit, .. }
            | Self::KeysetAfter { limit, .. }
            | Self::ServerCursor { limit, .. } => *limit,
        }
    }
}
