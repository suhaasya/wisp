//! Query execution UI state (LUM-033).

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

use gpui::{Entity, SharedString};
use tokio_util::sync::CancellationToken;
use gpui::AppContext;
use wisp_core::{
    bridge::DbCommandPayload, ColumnMeta, ConnectionId, DbBridge, DbEventPayload, FetchStrategy,
    GridBrowseSource,
    PagerConfig, ResultPager, RowCount, ScriptStatement, SessionSqlPageSource, SplitFlavor,
    StatementRunOutcome, WispError,
};

use crate::bridge::{db_bridge, spawn_db};
use crate::grid::DataGrid;

pub const MAX_RESULT_TABS_IN_MEMORY: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunScope {
    CurrentOrSelection,
    All,
}

#[derive(Debug, Clone)]
pub enum ResultPaneTab {
    Messages,
    Result {
        label: SharedString,
        sql: String,
        stmt_index: usize,
        columns: Vec<ColumnMeta>,
        row_count: RowCount,
    },
    Explain {
        label: SharedString,
        plan_text: String,
        stmt_index: usize,
    },
}

pub struct QueryRunState {
    pub running: bool,
    pub started_at: Option<Instant>,
    pub last_run_statements: Vec<ScriptStatement>,
    pub last_run_started: Option<Instant>,
    pub cancel: Option<CancellationToken>,
    pub pending_write_confirm: Option<String>,
    pub write_approved: bool,
    pub error_byte_range: Option<Range<usize>>,
    pub message_lines: Vec<String>,
    pub result_tabs: Vec<ResultPaneTab>,
    pub active_result_tab: usize,
    pub loaded_grids: HashMap<usize, Entity<DataGrid>>,
    pub result_counter: u32,
}

impl Default for QueryRunState {
    fn default() -> Self {
        Self {
            running: false,
            started_at: None,
            last_run_statements: Vec::new(),
            last_run_started: None,
            cancel: None,
            pending_write_confirm: None,
            write_approved: false,
            error_byte_range: None,
            message_lines: Vec::new(),
            result_tabs: vec![ResultPaneTab::Messages],
            active_result_tab: 0,
            loaded_grids: HashMap::new(),
            result_counter: 0,
        }
    }
}

impl QueryRunState {
    pub fn elapsed_ms(&self) -> u64 {
        self.started_at
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn apply_report(
        &mut self,
        outcomes: Vec<StatementRunOutcome>,
        cancelled: bool,
    ) {
        self.running = false;
        self.started_at = None;
        self.cancel = None;
        if cancelled {
            self.message_lines.push("Query cancelled.".into());
        }
        for outcome in outcomes {
            match outcome {
                StatementRunOutcome::Message { text, .. } => self.message_lines.push(text),
                StatementRunOutcome::Failed { message, byte_start, .. } => {
                    self.message_lines.push(format!("ERROR: {message}"));
                    self.error_byte_range = Some(byte_start..byte_start.saturating_add(1));
                }
                StatementRunOutcome::ResultSet {
                    stmt_index,
                    sql,
                    columns,
                    row_count,
                    ..
                } => {
                    self.result_counter += 1;
                    let label = SharedString::from(format!("Result {}", self.result_counter));
                    self.message_lines.push(format!(
                        "Statement {}: {} row(s) returned",
                        stmt_index + 1,
                        row_count_label(&row_count)
                    ));
                    self.result_tabs.push(ResultPaneTab::Result {
                        label,
                        sql,
                        stmt_index,
                        columns,
                        row_count,
                    });
                }
                StatementRunOutcome::Explain {
                    stmt_index,
                    plan_text,
                    ..
                } => {
                    self.result_counter += 1;
                    let label = SharedString::from(format!("Explain {}", self.result_counter));
                    self.message_lines
                        .push(format!("Statement {}: EXPLAIN plan ready", stmt_index + 1));
                    self.result_tabs.push(ResultPaneTab::Explain {
                        label,
                        plan_text,
                        stmt_index,
                    });
                }
            }
        }
        if !self.result_tabs.is_empty() {
            self.active_result_tab = self.result_tabs.len().saturating_sub(1);
        }
        self.trim_loaded_grids();
    }

    fn trim_loaded_grids(&mut self) {
        if self.loaded_grids.len() <= MAX_RESULT_TABS_IN_MEMORY {
            return;
        }
        let keep: Vec<usize> = self
            .result_tabs
            .iter()
            .enumerate()
            .filter_map(|(ix, tab)| {
                if matches!(tab, ResultPaneTab::Result { .. }) {
                    Some(ix)
                } else {
                    None
                }
            })
            .rev()
            .take(MAX_RESULT_TABS_IN_MEMORY)
            .collect();
        self.loaded_grids
            .retain(|ix, _| keep.contains(ix) || *ix == self.active_result_tab);
    }

    pub fn grid_for_tab(
        &mut self,
        tab_ix: usize,
        connection_id: ConnectionId,
        bridge: Arc<DbBridge>,
        workspace: gpui::WeakEntity<crate::workspace::WorkspaceView>,
        cx: &mut gpui::Context<crate::sql_editor::view::SqlEditorView>,
    ) -> Option<Entity<DataGrid>> {
        if let Some(grid) = self.loaded_grids.get(&tab_ix) {
            return Some(grid.clone());
        }
        let ResultPaneTab::Result {
            sql,
            columns,
            row_count,
            label,
            ..
        } = self.result_tabs.get(tab_ix)?
        else {
            return None;
        };
        let fetcher = crate::bridge::sql_fetch::shared_sql_fetcher(
            bridge,
            connection_id,
            sql.clone(),
        );
        let source = GridBrowseSource::SessionSql(SessionSqlPageSource::new(
            columns.clone(),
            *row_count,
            fetcher,
        ));
        let pager = ResultPager::new(source, FetchStrategy::Offset, PagerConfig::default()).ok()?;
        let cols = columns.clone();
        let connection = workspace
            .read_with(cx, |ws, _| ws.connection_commit_meta())
            .ok()
            .flatten();
        let grid = cx.new(|cx| {
            DataGrid::new_session_query(label.clone(), cols, pager, workspace, connection, cx)
        });
        self.loaded_grids.insert(tab_ix, grid.clone());
        self.trim_loaded_grids();
        Some(grid)
    }
}

fn row_count_label(count: &RowCount) -> String {
    match count {
        RowCount::Exact(n) => n.to_string(),
        RowCount::Estimate { rows, .. } => format!("~{rows}"),
        RowCount::Unknown => "?".into(),
    }
}

pub fn plan_statements(
    text: &str,
    scope: RunScope,
    split_flavor: SplitFlavor,
    selection_bytes: Option<Range<usize>>,
    caret_byte: usize,
) -> Vec<ScriptStatement> {
    use wisp_core::{current_statement_range, split_statements};
    let all = split_statements(text, split_flavor);
    let pick = match scope {
        RunScope::All => all,
        RunScope::CurrentOrSelection => {
            if let Some(sel) = selection_bytes.filter(|r| !r.is_empty()) {
                all.into_iter()
                    .filter(|r| r.start < sel.end && r.end > sel.start)
                    .collect()
            } else {
                vec![current_statement_range(text, caret_byte, split_flavor)]
            }
        }
    };
    pick.into_iter()
        .filter_map(|range| {
            let sql = text[range.clone()].to_string();
            if sql.trim().is_empty() {
                None
            } else {
                Some(ScriptStatement {
                    byte_start: range.start,
                    sql,
                })
            }
        })
        .collect()
}

pub fn dispatch_run_script<T: 'static>(
    cx: &mut gpui::Context<T>,
    connection_id: ConnectionId,
    statements: Vec<ScriptStatement>,
    write_approved: bool,
    on_done: impl FnOnce(&mut T, &mut gpui::Context<T>, Result<DbEventPayload, WispError>) + 'static,
) -> CancellationToken {
    let bridge = db_bridge(cx);
    let (_id, cancel, _task) = spawn_db(
        cx,
        &bridge,
        DbCommandPayload::SessionRunScript {
            id: connection_id,
            statements,
            write_approved,
        },
        on_done,
    );
    cancel
}

pub fn request_cancel(connection_id: ConnectionId, cx: &mut gpui::App) {
    let bridge = db_bridge(cx);
    let (_id, _cancel, _task) = bridge.submit(DbCommandPayload::SessionCancelQuery(connection_id));
}
