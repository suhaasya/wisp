//! Bounded ring-buffer pager: viewport sync, prefetch, invalidation.

use wisp_drivers::{Page, Value};

use super::config::PagerConfig;
use super::count::RowCount;
use super::fetch::PageFetch;
use super::ring::{CachedPage, PageRing};
use super::source::PageSource;
use super::strategy::{FetchStrategy, KeysetCursor};

#[derive(Debug, thiserror::Error)]
pub enum PagerError {
    #[error("page fetch failed: {0}")]
    Driver(#[from] wisp_drivers::DriverError),
    #[error(
        "result exceeds the {cap} byte tab limit ({used} bytes loaded); narrow the query or filter"
    )]
    MemoryCap { cap: usize, used: usize },
    #[error("row {row} is not loaded")]
    RowNotLoaded { row: u64 },
    #[error("pager invalidated")]
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub first_row: u64,
    pub visible_rows: u32,
}

#[derive(Debug)]
pub struct ResultPager<S: PageSource> {
    config: PagerConfig,
    source: S,
    strategy: FetchStrategy,
    generation: u64,
    filter_generation: u64,
    ring: PageRing,
    row_count: RowCount,
    viewport: Viewport,
    memory_used: usize,
    memory_cap_hit: bool,
}

impl<S: PageSource> ResultPager<S> {
    pub fn new(source: S, strategy: FetchStrategy, config: PagerConfig) -> Result<Self, PagerError> {
        let mut pager = Self {
            config,
            source,
            strategy,
            generation: 1,
            filter_generation: 0,
            ring: PageRing::new(config.ring_pages),
            row_count: RowCount::Unknown,
            viewport: Viewport {
                first_row: 0,
                visible_rows: config.rows_per_page,
            },
            memory_used: 0,
            memory_cap_hit: false,
        };
        pager.refresh_row_count_hint();
        Ok(pager)
    }

    pub fn config(&self) -> &PagerConfig {
        &self.config
    }

    pub fn strategy(&self) -> &FetchStrategy {
        &self.strategy
    }

    pub fn row_count(&self) -> RowCount {
        self.row_count
    }

    pub fn memory_cap_message(&self) -> Option<&str> {
        if self.memory_cap_hit {
            Some(
                "This tab hit the 64 MB result buffer limit. Apply a filter or close other tabs.",
            )
        } else {
            None
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn invalidate_all(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.ring.clear();
        self.memory_used = 0;
        self.memory_cap_hit = false;
        self.refresh_row_count_hint();
    }

    /// Drop cached result pages (background tab eviction).
    pub fn unload_pages(&mut self) {
        self.invalidate_all();
    }

    pub fn loaded_bytes(&self) -> usize {
        self.memory_used
    }

    pub fn invalidate_for_filter_or_sort(&mut self, filter_generation: u64) {
        if filter_generation != self.filter_generation {
            self.filter_generation = filter_generation;
            self.invalidate_all();
        }
    }

    pub fn invalidate_after_commit(&mut self) {
        self.invalidate_all();
    }

    pub fn request_exact_row_count(&mut self) -> Result<u64, PagerError> {
        let exact = self.source.fetch_count_exact()?;
        self.row_count = RowCount::Exact(exact);
        Ok(exact)
    }

    pub fn set_viewport(&mut self, viewport: Viewport) -> Result<(), PagerError> {
        self.viewport = viewport;
        self.sync_pages()?;
        Ok(())
    }

    /// Jump so `row` is near the top of the viewport (used for scroll-to-end).
    pub fn jump_to_row(&mut self, row: u64) -> Result<(), PagerError> {
        self.viewport.first_row = row;
        self.sync_pages()?;
        Ok(())
    }

    pub fn cell_text(&self, row: u64, col: usize) -> Result<Option<String>, PagerError> {
        let page_index = row / self.config.page_span_rows();
        let page = self
            .ring
            .get(page_index)
            .ok_or(PagerError::RowNotLoaded { row })?;
        let local = (row % self.config.page_span_rows()) as usize;
        Ok(page
            .text_at(local, col)
            .map(|s| s.to_string()))
    }

    pub fn total_rows(&self) -> u64 {
        self.row_count.best_guess().unwrap_or(0)
    }

    pub fn cell_value(&self, row: u64, col: usize) -> Result<Option<Value>, PagerError> {
        let page_index = row / self.config.page_span_rows();
        let page = self
            .ring
            .get(page_index)
            .ok_or(PagerError::RowNotLoaded { row })?;
        let local = (row % self.config.page_span_rows()) as usize;
        Ok(page.value(local, col).cloned())
    }

    pub fn row_id(&self, row: u64) -> Result<i64, PagerError> {
        let page_index = row / self.config.page_span_rows();
        let page = self
            .ring
            .get(page_index)
            .ok_or(PagerError::RowNotLoaded { row })?;
        let local = (row % self.config.page_span_rows()) as usize;
        match page.value(local, 0) {
            Some(Value::Int(id)) => Ok(*id),
            _ => Err(PagerError::RowNotLoaded { row }),
        }
    }

    fn refresh_row_count_hint(&mut self) {
        self.row_count = self.source.row_count_hint();
    }

    fn sync_pages(&mut self) -> Result<(), PagerError> {
        if self.memory_cap_hit {
            return Err(PagerError::MemoryCap {
                cap: self.config.memory_cap_bytes,
                used: self.memory_used,
            });
        }
        let last_visible = self
            .viewport
            .first_row
            .saturating_add(self.viewport.visible_rows as u64)
            .saturating_sub(1);
        let first_page = self.page_index_for_row(self.viewport.first_row);
        let last_page = self.page_index_for_row(last_visible);
        let center = self.page_index_for_row(self.viewport.first_row);
        let half = (self.config.ring_pages / 2) as u64;
        let mut want: Vec<u64> = (first_page..=last_page).collect();
        for delta in 0..=half {
            if center >= delta {
                want.push(center - delta);
            }
            want.push(center + delta);
        }
        want.sort_unstable();
        want.dedup();
        for page_index in want {
            if !self.ring.contains(page_index) {
                self.load_page(page_index)?;
            }
        }
        self.prefetch_from_viewport();
        Ok(())
    }

    fn viewport_page_span(&self) -> (u64, u64) {
        let last_visible = self
            .viewport
            .first_row
            .saturating_add(self.viewport.visible_rows as u64)
            .saturating_sub(1);
        (
            self.page_index_for_row(self.viewport.first_row),
            self.page_index_for_row(last_visible),
        )
    }

    fn prefetch_from_viewport(&mut self) {
        if self.memory_cap_hit {
            return;
        }
        let last_visible = self
            .viewport
            .first_row
            .saturating_add(self.viewport.visible_rows as u64)
            .saturating_sub(1);
        let first_page = self.page_index_for_row(self.viewport.first_row);
        let last_page = self.page_index_for_row(last_visible);
        let threshold = self.config.prefetch_pages as u64;
        let prefetch_below = self
            .viewport
            .first_row
            .saturating_sub(first_page * self.config.page_span_rows())
            < threshold * self.config.page_span_rows();
        let prefetch_above = last_visible
            .saturating_sub(last_page * self.config.page_span_rows())
            > self
                .config
                .page_span_rows()
                .saturating_sub(threshold * self.config.page_span_rows());
        if prefetch_above {
            let next = last_page.saturating_add(1);
            if !self.ring.contains(next) {
                let _ = self.load_page(next);
            }
        }
        if prefetch_below && first_page > 0 {
            let prev = first_page - 1;
            if !self.ring.contains(prev) {
                let _ = self.load_page(prev);
            }
        }
    }

    fn page_index_for_row(&self, row: u64) -> u64 {
        row / self.config.page_span_rows()
    }

    fn load_page(&mut self, page_index: u64) -> Result<(), PagerError> {
        if self.ring.contains(page_index) {
            return Ok(());
        }
        let offset = page_index * self.config.page_span_rows();
        let fetch = self.plan_fetch(offset, page_index);
        let page = self.source.fetch_page(fetch)?;
        self.install_page(page_index, offset, page)?;
        Ok(())
    }

    fn plan_fetch(&self, offset: u64, page_index: u64) -> PageFetch {
        let limit = self.config.rows_per_page;
        match &self.strategy {
            FetchStrategy::Keyset(_plan) if page_index > 0 => {
                if let Some(cursor) = self.ring.end_cursor(page_index - 1) {
                    PageFetch::KeysetAfter { cursor, limit }
                } else {
                    PageFetch::Offset { offset, limit }
                }
            }
            FetchStrategy::ServerCursor => PageFetch::ServerCursor {
                cursor_id: page_index,
                limit,
            },
            FetchStrategy::Keyset(_) | FetchStrategy::Offset => PageFetch::Offset { offset, limit },
        }
    }

    fn install_page(
        &mut self,
        page_index: u64,
        offset: u64,
        page: Page,
    ) -> Result<(), PagerError> {
        let bytes = page.approx_bytes();
        let anchor = self.page_index_for_row(self.viewport.first_row);
        let (protect_from, protect_through) = self.viewport_page_span();
        let prior = self.ring.resident_bytes();
        let evicted = self
            .ring
            .would_evict_bytes(anchor, protect_from, protect_through);
        let projected = prior.saturating_sub(evicted).saturating_add(bytes);
        if projected > self.config.memory_cap_bytes {
            self.memory_cap_hit = true;
            return Err(PagerError::MemoryCap {
                cap: self.config.memory_cap_bytes,
                used: prior,
            });
        }
        let end_cursor = page.row_count.checked_sub(1).and_then(|last_row| {
            page.value(last_row, 0)
                .map(|id| KeysetCursor {
                    values: vec![id.clone()],
                })
        });
        self.ring.insert(
            CachedPage {
                page_index,
                offset,
                page,
                bytes,
                end_cursor,
            },
            anchor,
            protect_from,
            protect_through,
        );
        self.memory_used = self.ring.resident_bytes();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::pager::{KeysetPlan, SequentialPageSource};

    fn keyset_strategy() -> FetchStrategy {
        FetchStrategy::Keyset(KeysetPlan {
            order_columns: vec!["id".into()],
            primary_key: "id".into(),
        })
    }

    #[test]
    fn ring_keeps_three_pages_around_viewport() {
        let source = SequentialPageSource::new(5_000, 2);
        let mut pager = ResultPager::new(
            source,
            FetchStrategy::Offset,
            PagerConfig::default(),
        )
        .expect("pager");
        pager
            .set_viewport(Viewport {
                first_row: 900,
                visible_rows: 50,
            })
            .expect("sync");
        assert_eq!(pager.row_id(950).expect("row 950"), 950);
        assert_eq!(pager.row_id(750).expect("row 750"), 750);
        assert_eq!(pager.row_id(1050).expect("row 1050"), 1050);
    }

    #[test]
    fn jump_to_end_is_fast_on_keyset_fixture() {
        let source = SequentialPageSource::new(1_000_000, 2);
        let mut pager = ResultPager::new(source, keyset_strategy(), PagerConfig::default())
            .expect("pager");
        let start = Instant::now();
        pager.jump_to_row(999_950).expect("jump");
        assert!(start.elapsed().as_millis() < 300);
        assert_eq!(pager.row_id(999_999).expect("id"), 999_999);
    }

    #[test]
    fn no_duplicate_rows_across_page_boundary() {
        let source = SequentialPageSource::new(1_000, 1);
        let mut pager = ResultPager::new(source, keyset_strategy(), PagerConfig::default())
            .expect("pager");
        pager
            .set_viewport(Viewport {
                first_row: 295,
                visible_rows: 20,
            })
            .expect("sync");
        let mut seen = std::collections::BTreeSet::new();
        for row in 295..315 {
            let id = pager.row_id(row).expect("loaded");
            assert!(seen.insert(id), "duplicate id {id} at row {row}");
        }
    }

    #[test]
    fn repro_proptest_row_600() {
        let source = SequentialPageSource::new(100_000, 2);
        let strategy = FetchStrategy::Keyset(KeysetPlan {
            order_columns: vec!["id".into()],
            primary_key: "id".into(),
        });
        let mut pager = ResultPager::new(source, strategy, PagerConfig::default()).expect("pager");
        let mut row = 0u64;
        for _ in 0..14 {
            pager
                .set_viewport(Viewport {
                    first_row: row,
                    visible_rows: 120,
                })
                .expect("viewport");
            for offset in 0..120u32 {
                let global = row.saturating_add(offset as u64);
                if global >= 100_000 {
                    break;
                }
                pager.row_id(global).expect("row {global}");
            }
            row = row.saturating_add(38);
        }
    }

    #[test]
    fn memory_cap_surfaces_message() {
        let source = SequentialPageSource::new(10_000, 4);
        let mut config = PagerConfig::default();
        config.memory_cap_bytes = 1024;
        let mut pager =
            ResultPager::new(source, FetchStrategy::Offset, config).expect("pager");
        let err = pager
            .set_viewport(Viewport {
                first_row: 0,
                visible_rows: 300,
            })
            .expect_err("cap");
        assert!(matches!(err, PagerError::MemoryCap { .. }));
        assert!(pager.memory_cap_message().is_some());
    }
}

