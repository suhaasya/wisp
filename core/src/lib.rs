//! Session lifecycle, result paging, and query execution orchestration.
//!
//! Owns the domain model for connections, running queries, and presenting result sets.
//! May depend on [`wisp_drivers`] and [`wisp_store`]. Must not depend on UI or the
//! binary crate.

pub mod bridge;
pub mod connections;
pub mod error;
pub mod grid;
pub mod render;

pub use connections::{
    card_from_profile, ConnectionBadge, ConnectionCardView, ConnectionEngine,
    ConnectionEngineKind, ConnectionFolder, ConnectionGroup, ConnectionHub,
    ConnectionHubError, ConnectionId, ConnectionManagerController, ConnectionProfile,
    ConnectionsView, EnvFilter, EnvironmentTag, ManagerAction, RailSelection, TransportKind,
};
pub use error::{WispError, WispErrorKind};
pub use bridge::{
    assert_no_block_on_ui, enter_runtime_thread, enter_ui_thread, DbBridge, DbCommandPayload,
    DbEvent, DbEventPayload, DbRuntimeConfig, RequestId,
};
pub use wisp_drivers::{
    ColumnMeta, DbDriver, Dialect, DriverError, EngineKind, ExecuteStats, MockDriver,
    MysqlConfig, MysqlDialect, MysqlDriver, Page, PageArena, PageBuilder, PageRequest,
    PostgresConfig, PostgresDialect, PostgresDriver, QueryId, ServerInfo, Value,
    BYTES_PREVIEW_MAX,
};

/// Placeholder until core session logic lands in later milestones.
pub const CRATE_MARKER: &str = "wisp-core";

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(super::CRATE_MARKER, "wisp-core");
    }
}
