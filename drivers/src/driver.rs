//! Object-safe database driver interface (engine adapters in LUM-012 / LUM-013).

use crate::{
    dialect::Dialect,
    error::DriverError,
    page::Page,
};

/// Which engine a driver implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    PostgreSql,
    MySql,
    MariaDb,
    Mock,
}

/// Static server description after connect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    pub engine: EngineKind,
    pub version: String,
    pub database: String,
    pub user: String,
}

/// Window into a result set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    pub limit: u32,
    pub offset: u64,
}

/// Rows changed / returned metadata for `execute`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExecuteStats {
    pub rows_affected: u64,
}

/// Handle for in-flight work (`cancel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueryId(pub u64);

impl QueryId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Engine-neutral driver surface. Implementations must be `Send` and may be used behind
/// `Box<dyn DbDriver>` on the tokio runtime (see LUM-010).
///
/// # Examples
///
/// ```no_run
/// use wisp_drivers::{DbDriver, MockDriver, PageRequest};
///
/// let mut driver = MockDriver::default();
/// driver.connect().expect("connect");
/// let page = driver
///     .query_paged("SELECT * FROM users", PageRequest { limit: 100, offset: 0 })
///     .expect("page");
/// assert!(page.row_count > 0);
/// driver.close().expect("close");
/// ```
pub trait DbDriver: Send {
    fn connect(&mut self) -> Result<(), DriverError>;

    fn close(&mut self) -> Result<(), DriverError>;

    fn ping(&mut self) -> Result<(), DriverError>;

    fn execute(&mut self, sql: &str) -> Result<ExecuteStats, DriverError>;

    fn query_paged(&mut self, sql: &str, request: PageRequest) -> Result<Page, DriverError>;

    fn cancel(&mut self, query: QueryId) -> Result<(), DriverError>;

    fn begin(&mut self) -> Result<(), DriverError>;

    fn commit(&mut self) -> Result<(), DriverError>;

    fn rollback(&mut self) -> Result<(), DriverError>;

    fn server_info(&self) -> Result<ServerInfo, DriverError>;

    fn dialect(&self) -> &dyn Dialect;
}
