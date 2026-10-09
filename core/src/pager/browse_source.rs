//! Unified browse pager source (sequential bench vs shop mock tables).

use wisp_drivers::{ColumnMeta, DriverError, Page};

use super::fetch::PageFetch;
use super::queryable::QueryableSequentialSource;
use super::queryable_mock::QueryableMockTableSource;
use super::session_sql::SessionSqlPageSource;
use super::source::PageSource;
use super::count::RowCount;

#[derive(Clone)]
pub enum GridBrowseSource {
    Sequential(QueryableSequentialSource),
    MockTable(QueryableMockTableSource),
    SessionSql(SessionSqlPageSource),
}

impl PageSource for GridBrowseSource {
    fn columns(&self) -> Vec<ColumnMeta> {
        match self {
            Self::Sequential(s) => s.columns(),
            Self::MockTable(s) => s.columns(),
            Self::SessionSql(s) => s.columns(),
        }
    }

    fn row_count_hint(&mut self) -> RowCount {
        match self {
            Self::Sequential(s) => s.row_count_hint(),
            Self::MockTable(s) => s.row_count_hint(),
            Self::SessionSql(s) => s.row_count_hint(),
        }
    }

    fn fetch_count_exact(&mut self) -> Result<u64, DriverError> {
        match self {
            Self::Sequential(s) => s.fetch_count_exact(),
            Self::MockTable(s) => s.fetch_count_exact(),
            Self::SessionSql(s) => s.fetch_count_exact(),
        }
    }

    fn fetch_page(&mut self, request: PageFetch) -> Result<Page, DriverError> {
        match self {
            Self::Sequential(s) => s.fetch_page(request),
            Self::MockTable(s) => s.fetch_page(request),
            Self::SessionSql(s) => s.fetch_page(request),
        }
    }
}
