//! Pager tuning (ring size, prefetch, memory cap).

/// Default ring holds three pages of three hundred rows each (~900 rows visible window).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PagerConfig {
    pub ring_pages: usize,
    pub rows_per_page: u32,
    /// Prefetch when the viewport is within this many pages of a loaded edge.
    pub prefetch_pages: u32,
    /// Per-tab resident page bytes (default 64 MiB).
    pub memory_cap_bytes: usize,
    /// Tables with at most this many rows get an exact `COUNT(*)` up front.
    pub exact_count_threshold: u64,
}

impl Default for PagerConfig {
    fn default() -> Self {
        Self {
            ring_pages: 3,
            rows_per_page: 300,
            prefetch_pages: 1,
            memory_cap_bytes: 64 * 1024 * 1024,
            exact_count_threshold: 10_000,
        }
    }
}

impl PagerConfig {
    pub fn page_span_rows(&self) -> u64 {
        self.rows_per_page as u64
    }
}
