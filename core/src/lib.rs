//! Session lifecycle, result paging, and query execution orchestration.
//!
//! Owns the domain model for connections, running queries, and presenting result sets.
//! May depend on [`wisp_drivers`] and [`wisp_store`]. Must not depend on UI or the
//! binary crate.

pub mod bridge;
pub mod connections;
pub mod error;
pub mod schema;
pub mod session;
pub mod pager;
pub mod query;
pub mod grid;
pub mod render;
pub mod workspace;

pub use connections::{
    card_from_profile, validate, ConnectionBadge, ConnectionCardView, ConnectionEngine,
    ConnectionEngineKind, ConnectionFolder, ConnectionFormDraft, ConnectionGroup, ConnectionHub,
    ConnectionHubError, ConnectionId, ConnectionManagerController, ConnectionProfile,
    ConnectionTestError, ConnectionTestOutcome, ConnectionTestSpec, ConnectionsView, EnvFilter,
    EnvironmentTag, FormEngine, FormEnvironment, FormField, FormTab, FormValidation,
    ManagerAction, parse_connection_paste, RailSelection, SslMode, SslSettings, SslTrustStore,
    SshAuthMethod, TransportKind, UrlParseError,
};
pub use error::{WispError, WispErrorKind};
pub use schema::{
    build_sidebar_rows, demo_catalog_large_tables, demo_catalog_shop,
    fuzzy_match_highlight_indices, fuzzy_match_name,
    SchemaCatalog, SchemaObjectId, SchemaObjectKind, SchemaSidebarPrefs, SchemaSidebarStateStore,
    SidebarRow,
};
pub use pager::{
    DriverPageSource, FetchStrategy, FetchStrategyKind, KeysetCursor, KeysetPlan, PageFetch,
    PageSource, PagerConfig, PagerError, QueryableSequentialSource, ResultPager, RowCount,
    SequentialPageSource, Viewport, tuple_seek_predicate,
};
pub use query::{
    build_table_select, sql_for_display, FilterCombine, FilterModel, FilterOperator, FilterTerm,
    SortDirection, SortKey, SortModel, TableDataQuery, BuiltSql, SqlParam,
};
pub use grid::{bench_scroll_1m, GridScrollBenchResult, GridStatus};
pub use workspace::{bench_tabs_20, SavedWorkspaceTab, SavedWorkspaceTabKind, Tabs20BenchResult, WorkspaceSessionStore, WorkspaceTabState};
pub use session::{
    check_write_allowed, SessionError, SessionEvent, SessionManager, SessionOpenSpec, SessionPhase,
    SessionRuntimeConfig, SessionSecrets, SessionSnapshot, sql_looks_like_write,
};
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
