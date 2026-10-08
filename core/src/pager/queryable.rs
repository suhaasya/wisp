//! Page source that applies server-side filter/sort before paging (mock execution).

use wisp_drivers::{ColumnMeta, DriverError, Page, PageBuilder, Value};

use crate::query::{row_matches_filter, sort_row_ids, TableDataQuery};

use super::count::RowCount;
use super::fetch::PageFetch;
use super::strategy::KeysetCursor;
use super::source::PageSource;
use super::source::SequentialPageSource;

#[derive(Debug, Clone)]
pub struct QueryableSequentialSource {
    inner: SequentialPageSource,
    query: TableDataQuery,
    matching_ids: Vec<u64>,
}

impl QueryableSequentialSource {
    pub fn new(inner: SequentialPageSource, query: TableDataQuery) -> Self {
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
        let total = self.inner.base_row_count();
        let mut ids: Vec<u64> = (0..total)
            .filter(|&id| {
                row_matches_filter(&self.query.filter, |col| self.column_text(id, col))
            })
            .collect();
        ids = sort_row_ids(ids, &self.query.sort, |id, col| self.column_text(id, col));
        self.matching_ids = ids;
    }

    fn column_text(&self, id: u64, column: &str) -> Option<String> {
        if column == "id" {
            return Some(id.to_string());
        }
        if let Some(rest) = column.strip_prefix("col_") {
            let _c: usize = rest.parse().ok()?;
            return Some(format!("r{id}c{rest}"));
        }
        None
    }

    fn build_page_for_ids(&self, ids: &[u64]) -> Result<Page, DriverError> {
        if ids.is_empty() {
            return Ok(PageBuilder::new(PageSource::columns(&self.inner)).finish());
        }
        let mut builder = PageBuilder::new(PageSource::columns(&self.inner));
        for &id in ids {
            let mut cells = vec![Value::Int(id as i64)];
            for c in 0..self.inner.payload_columns() {
                cells.push(Value::Text(
                    builder
                        .arena_mut()
                        .push_str(&format!("r{id}c{c}")),
                ));
            }
            builder
                .push_row(cells)
                .map_err(|_| DriverError::InvalidPage)?;
        }
        Ok(builder.finish())
    }
}

impl PageSource for QueryableSequentialSource {
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
                    "queryable mock",
                ));
            }
        };
        let start = offset as usize;
        if start >= self.matching_ids.len() {
            return self.build_page_for_ids(&[]);
        }
        let end = start.saturating_add(limit as usize).min(self.matching_ids.len());
        let slice = &self.matching_ids[start..end];
        self.build_page_for_ids(slice)
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
