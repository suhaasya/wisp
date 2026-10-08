//! Database protocol drivers (PostgreSQL, MySQL, and future engines).
//!
//! Owns the engine-neutral [`DbDriver`] trait, result [`Page`] layout, and wire adapters
//! (LUM-012 / LUM-013). May depend on [`wisp_transport`] only among Wisp crates.

mod arena;
mod column;
mod dialect;
mod driver;
mod error;
mod mock;
mod page;
mod mysql;
mod postgres;
mod value;

pub use arena::{BlobRef, StrRef};
pub use column::ColumnMeta;
pub use dialect::{Dialect, MysqlDialect, PostgresDialect};
pub use driver::{
    DbDriver, EngineKind, ExecuteStats, PageRequest, QueryId, ServerInfo,
};
pub use error::DriverError;
pub use mock::MockDriver;
pub use mysql::{MysqlConfig, MysqlDriver};
pub use postgres::{PostgresConfig, PostgresDriver};
pub use page::{Page, PageArena, PageBuilder, PageError};
pub use value::{BytesPreview, Value, BYTES_PREVIEW_MAX};

pub const CRATE_MARKER: &str = "wisp-drivers";

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(super::CRATE_MARKER, "wisp-drivers");
    }
}
