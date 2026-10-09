//! Backing store for page fetches (mock sequential rows, driver adapter).

use wisp_drivers::{ColumnMeta, DriverError, Page, PageBuilder, Value};

use super::count::RowCount;
use super::fetch::PageFetch;
use super::strategy::KeysetCursor;

/// Loads one page of a tabular result.
pub trait PageSource: Send {
    fn row_count_hint(&mut self) -> RowCount;

    fn fetch_count_exact(&mut self) -> Result<u64, DriverError>;

    fn fetch_page(&mut self, request: PageFetch) -> Result<Page, DriverError>;

    fn columns(&self) -> Vec<ColumnMeta> {
        Vec::new()
    }
}

/// Deterministic `id` + payload columns for tests and property tests.
#[derive(Debug, Clone)]
pub struct SequentialPageSource {
    total_rows: u64,
    payload_columns: usize,
    exact_count_threshold: u64,
}

impl SequentialPageSource {
    pub fn new(total_rows: u64, payload_columns: usize) -> Self {
        Self {
            total_rows,
            payload_columns,
            exact_count_threshold: 10_000,
        }
    }

    pub fn with_exact_threshold(mut self, threshold: u64) -> Self {
        self.exact_count_threshold = threshold;
        self
    }

    pub fn base_row_count(&self) -> u64 {
        self.total_rows
    }

    pub fn payload_columns(&self) -> usize {
        self.payload_columns
    }

    fn columns(&self) -> Vec<ColumnMeta> {
        let mut id = ColumnMeta::new("id", "bigint");
        id.is_pk = true;
        let mut cols = vec![id];
        for c in 0..self.payload_columns {
            cols.push(ColumnMeta::new(format!("col_{c}"), "text"));
        }
        cols
    }

    fn build_page(&self, start_id: u64, limit: u32) -> Result<Page, DriverError> {
        if limit == 0 {
            return Err(DriverError::InvalidPage);
        }
        let mut builder = PageBuilder::new(self.columns());
        let end = start_id.saturating_add(limit as u64).min(self.total_rows);
        for id in start_id..end {
            let mut cells = vec![Value::Int(id as i64)];
            for c in 0..self.payload_columns {
                cells.push(Value::Text(
                    builder
                        .arena_mut()
                        .push_str(&format!("r{id}c{c}")),
                ));
            }
            builder.push_row(cells).map_err(|_| DriverError::InvalidPage)?;
        }
        Ok(builder.finish())
    }

    fn offset_from_cursor(cursor: &KeysetCursor) -> Result<u64, DriverError> {
        let Some(Value::Int(id)) = cursor.values.first() else {
            return Err(DriverError::user(
                "Invalid keyset cursor",
                "expected Int primary key",
            ));
        };
        if *id < 0 {
            return Err(DriverError::user(
                "Invalid keyset cursor",
                "negative primary key",
            ));
        }
        Ok((*id as u64).saturating_add(1))
    }
}

impl PageSource for SequentialPageSource {
    fn columns(&self) -> Vec<ColumnMeta> {
        let mut cols = vec![ColumnMeta::new("id", "bigint")];
        for c in 0..self.payload_columns {
            cols.push(ColumnMeta::new(format!("col_{c}"), "text"));
        }
        cols
    }
    fn row_count_hint(&mut self) -> RowCount {
        if self.total_rows <= self.exact_count_threshold {
            RowCount::Exact(self.total_rows)
        } else {
            RowCount::Estimate {
                rows: self.total_rows,
                source: "mock_statistics",
            }
        }
    }

    fn fetch_count_exact(&mut self) -> Result<u64, DriverError> {
        Ok(self.total_rows)
    }

    fn fetch_page(&mut self, request: PageFetch) -> Result<Page, DriverError> {
        match request {
            PageFetch::Offset { offset, limit } => self.build_page(offset, limit),
            PageFetch::KeysetAfter { cursor, limit } => {
                let start = Self::offset_from_cursor(&cursor)?;
                self.build_page(start, limit)
            }
            PageFetch::ServerCursor { .. } => Err(DriverError::user(
                "Server cursor not supported",
                "mock source",
            )),
        }
    }
}

/// Wraps [`wisp_drivers::DbDriver`] offset paging (keyset wired when drivers expose it).
pub struct DriverPageSource<'a, D: wisp_drivers::DbDriver + ?Sized> {
    driver: &'a mut D,
    sql: String,
    count_hint: RowCount,
}

impl<'a, D: wisp_drivers::DbDriver + ?Sized> DriverPageSource<'a, D> {
    pub fn new(driver: &'a mut D, sql: impl Into<String>, count_hint: RowCount) -> Self {
        Self {
            driver,
            sql: sql.into(),
            count_hint,
        }
    }
}

impl<D: wisp_drivers::DbDriver + ?Sized> PageSource for DriverPageSource<'_, D> {
    fn row_count_hint(&mut self) -> RowCount {
        self.count_hint
    }

    fn fetch_count_exact(&mut self) -> Result<u64, DriverError> {
        let stats = self.driver.execute(&format!("SELECT COUNT(*) FROM ({}) q", self.sql))?;
        Ok(stats.rows_affected)
    }

    fn fetch_page(&mut self, request: PageFetch) -> Result<Page, DriverError> {
        match request {
            PageFetch::Offset { offset, limit } => self.driver.query_paged(
                &self.sql,
                wisp_drivers::PageRequest { limit, offset },
            ),
            PageFetch::KeysetAfter { .. } | PageFetch::ServerCursor { .. } => {
                Err(DriverError::user(
                    "Fetch strategy not implemented",
                    "driver adapter uses OFFSET until keyset SQL is wired",
                ))
            }
        }
    }
}
