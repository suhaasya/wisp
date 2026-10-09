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
pub mod sql_editor;
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
    build_sidebar_rows, catalog_disconnected, catalog_placeholder, demo_catalog_large_tables,
    demo_catalog_shop,
    fuzzy_match_highlight_indices, fuzzy_match_name,
    query_for_fk_target, relations_for_table, reverse_references, ForeignKey, ReverseReference,
    SchemaCatalog, SchemaLoadResult, SchemaMetadataCache, SchemaObjectId, SchemaObjectKind,
    SchemaSidebarPrefs, SchemaSidebarStateStore, SharedMetadataCache, SidebarRow, TableRelationSchema,
    load_schema_catalog, shared_metadata_cache,
};
pub use pager::{
    DriverPageSource, FetchStrategy, FetchStrategyKind, KeysetCursor, KeysetPlan, PageFetch,
    GridBrowseSource, PageSource, PagerConfig, PagerError, QueryableMockTableSource,
    QueryableSequentialSource, ResultPager, RowCount, SessionSqlFetcher, SessionSqlPageSource,
    SequentialPageSource, Viewport, tuple_seek_predicate,
};
pub use query::{
    build_table_select, sql_for_display, FilterCombine, FilterModel, FilterOperator, FilterTerm,
    SortDirection, SortKey, SortModel, TableDataQuery, BuiltSql, SqlParam,
};
pub use grid::{
    bench_scroll_1m, mock_cell_display, mock_cell_needs_lazy_fetch, mock_cell_preview,
    mock_cell_value,
    mock_layout_for_table,
    pretty_format_value, preview_cell_text, CellEdit, CellStage, EffectiveCell, GridScrollBenchResult,
    build_commit_statements, sql_for_execution, CommitStatement, CommitStatementKind,
    GridCommitSpec, GridStatus, InsertedRow, PrimaryKey, RowKey,
    RowStage, TableChangeSet,
    MockTablePageSource, RowDetailField, LARGE_VALUE_THRESHOLD,
};
pub use workspace::{bench_tabs_20, SavedWorkspaceTab, SavedWorkspaceTabKind, Tabs20BenchResult, WorkspaceSessionStore, WorkspaceTabState};
pub use sql_editor::{
    current_statement_range, split_statements, spans_for_line, statement_index_at_cursor,
    complete_at_cursor, CompletionContext, CompletionItem, CompletionItemKind, format_explain_json,
    SnippetSuggestion,
    HighlightSpan, RecentCompletionUse, Selection, SplitFlavor, sql_is_explain,
    sql_is_explain_analyze, sql_returns_rows, SqlDialect, SqlEditor, SqlSyntaxHighlighter,
    SyntaxTokenKind,
};
pub use session::{
    check_write_allowed, CommitTransactionOutcome, QueryPageSnapshot, RunScriptReport,
    ScriptStatement, SessionError, SessionEvent, SessionManager, SessionOpenSpec, SessionPhase,
    SessionRuntimeConfig, SessionSecrets, SessionSnapshot, StatementRunOutcome,
    sql_looks_like_write,
};
pub use bridge::{
    assert_no_block_on_ui, enter_runtime_thread, enter_ui_thread, DbBridge, DbCommandPayload,
    DbEvent, DbEventPayload, DbRuntimeConfig, RequestId,
};
pub use wisp_store::{
    append_history_entry, discard_pending, json_has_credential_keys, list_pending_journals,
    read_journal, remove_journal, search_history_file, write_journal_atomic, JournalCellEdit,
    JournalInsertedRow, JournalRowKey, JournalTab, JournalTabKind, JournalWorkspace, JournalWriter,
    QueryHistoryEntry, QueryHistoryStatus, Snippet, SnippetStore, SnippetStoreError,
    StagedCellEditRow, StagedGridSnapshot, WindowJournal, JOURNAL_VERSION,
    WispPaths,
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
