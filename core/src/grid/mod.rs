//! Scrollable result grid (hot path — compiled at `opt-level = 3` via crate override).

mod bench;
mod change_set;
mod journal_snap;
mod commit_sql;
mod mock_table;
mod row_detail;
mod status;

pub use bench::{bench_scroll_1m, GridScrollBenchResult};
pub use change_set::{
    CellEdit, CellStage, EffectiveCell, InsertedRow, PrimaryKey, RowKey, RowStage, TableChangeSet,
};
pub use commit_sql::{
    build_commit_statements, sql_for_execution, CommitStatement, CommitStatementKind,
    GridCommitSpec,
};
pub use mock_table::{
    mock_cell_display, mock_cell_needs_lazy_fetch, mock_cell_preview, mock_cell_value,
    mock_layout_for_table, MockTablePageSource,
};
pub use row_detail::{
    pretty_format_value, preview_cell_text, RowDetailField, LARGE_VALUE_THRESHOLD,
};
pub use crate::pager::{PagerConfig, ResultPager, Viewport};
pub use status::GridStatus;
pub use wisp_store::StagedGridSnapshot;

pub const MODULE_MARKER: &str = "wisp-core-grid";
