//! Filter/sort paging over [`MockTablePageSource`] (LUM-027 shop tables).

use wisp_drivers::{ColumnMeta, DriverError, Page, PageBuilder, Value};

use crate::grid::{mock_cell_preview, mock_cell_value, MockTablePageSource};
use crate::query::{row_matches_filter, sort_row_ids, TableDataQuery};

use super::count::RowCount;
use super::fetch::PageFetch;
use super::source::PageSource;
use super::strategy::KeysetCursor;

#[derive(Debug, Clone)]
pub struct QueryableMockTableSource {
    inner: MockTablePageSource,
    query: TableDataQuery,
    matching_ids: Vec<u64>,
}

impl QueryableMockTableSource {
    pub fn new(inner: MockTablePageSource, query: TableDataQuery) -> Self {
        let mut source = Self {
            inner,
            query,
            matching_ids: Vec::new(),
        };
        source.rebuild_matching();
        source
    }

    pub fn set_query(&mut self, query: TableDataQuery) {
        self.query = query;
        self.rebuild_matching();
    }

    pub fn query(&self) -> &TableDataQuery {
        &self.query
    }

    fn rebuild_matching(&mut self) {
        let table = self.inner.layout().table.clone();
        let total = self.inner.total_rows;
        let layout = self.inner.layout().clone();
        let mut ids: Vec<u64> = (0..total)
            .filter(|&id| {
                row_matches_filter(&self.query.filter, |col| {
                    layout
                        .columns
                        .iter()
                        .find(|c| c.meta.name == col)
                        .map(|c| mock_cell_preview(&table, id, c))
                })
            })
            .collect();
        ids = sort_row_ids(ids, &self.query.sort, |id, col| {
            layout
                .columns
                .iter()
                .find(|c| c.meta.name == col)
                .map(|c| mock_cell_preview(&table, id, c))
        });
        self.matching_ids = ids;
    }

    fn build_page_for_ids(&self, ids: &[u64]) -> Result<Page, DriverError> {
        if ids.is_empty() {
            return Ok(PageBuilder::new(PageSource::columns(&self.inner)).finish());
        }
        let table = self.inner.layout().table.clone();
        let layout = self.inner.layout();
        let mut builder = PageBuilder::new(PageSource::columns(&self.inner));
        for &id in ids {
            let mut cells = Vec::with_capacity(layout.columns.len());
            for c in &layout.columns {
                let arena = builder.arena_mut();
                cells.push(mock_cell_value(&table, id, c, arena));
            }
            builder.push_row(cells).map_err(|_| DriverError::InvalidPage)?;
        }
        Ok(builder.finish())
    }
}

impl PageSource for QueryableMockTableSource {
    fn columns(&self) -> Vec<ColumnMeta> {
        self.inner.columns()
    }

    fn row_count_hint(&mut self) -> RowCount {
        RowCount::Exact(self.matching_ids.len() as u64)
    }

    fn fetch_count_exact(&mut self) -> Result<u64, DriverError> {
        Ok(self.matching_ids.len() as u64)
    }

    fn fetch_page(&mut self, request: PageFetch) -> Result<Page, DriverError> {
        let (offset, limit) = match request {
            PageFetch::Offset { offset, limit } => (offset, limit),
            PageFetch::KeysetAfter { cursor, limit } => {
                let start = keyset_start(&cursor)?;
                (start, limit)
            }
            PageFetch::ServerCursor { .. } => {
                return Err(DriverError::user(
                    "Server cursor not supported",
                    "queryable mock table",
                ));
            }
        };
        let start = offset as usize;
        if start >= self.matching_ids.len() {
            return self.build_page_for_ids(&[]);
        }
        let end = start.saturating_add(limit as usize).min(self.matching_ids.len());
        self.build_page_for_ids(&self.matching_ids[start..end])
    }
}

fn keyset_start(cursor: &KeysetCursor) -> Result<u64, DriverError> {
    let Some(Value::Int(id)) = cursor.values.first() else {
        return Err(DriverError::user(
            "Invalid keyset cursor",
            "expected Int primary key",
        ));
    };
    Ok((*id).max(0) as u64)
}
