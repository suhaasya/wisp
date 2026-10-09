//! Pager source that fetches SQL result pages via a shared fetcher (LUM-033).

use std::sync::{Arc, Mutex};

use wisp_drivers::{ColumnMeta, DriverError, Page};

use super::count::RowCount;
use super::fetch::PageFetch;
use super::source::PageSource;

pub trait SessionSqlFetcher: Send {
    fn fetch_page(&mut self, offset: u64, limit: u32) -> Result<Page, DriverError>;
}

#[derive(Clone)]
pub struct SessionSqlPageSource {
    columns: Vec<ColumnMeta>,
    row_count: RowCount,
    fetcher: Arc<Mutex<dyn SessionSqlFetcher>>,
}

impl SessionSqlPageSource {
    pub fn new(
        columns: Vec<ColumnMeta>,
        row_count: RowCount,
        fetcher: Arc<Mutex<dyn SessionSqlFetcher>>,
    ) -> Self {
        Self {
            columns,
            row_count,
            fetcher,
        }
    }
}

impl PageSource for SessionSqlPageSource {
    fn columns(&self) -> Vec<ColumnMeta> {
        self.columns.clone()
    }

    fn row_count_hint(&mut self) -> RowCount {
        self.row_count
    }

    fn fetch_count_exact(&mut self) -> Result<u64, DriverError> {
        match self.row_count {
            RowCount::Exact(n) => Ok(n),
            _ => Err(DriverError::user(
                "Exact count unavailable",
                "ad-hoc query result",
            )),
        }
    }

    fn fetch_page(&mut self, request: PageFetch) -> Result<Page, DriverError> {
        let (offset, limit) = match request {
            PageFetch::Offset { offset, limit } => (offset, limit),
            PageFetch::KeysetAfter { .. } | PageFetch::ServerCursor { .. } => {
                return Err(DriverError::user(
                    "Fetch strategy not supported",
                    "session SQL uses OFFSET",
                ));
            }
        };
        self.fetcher
            .lock()
            .expect("session sql fetcher")
            .fetch_page(offset, limit)
    }
}
