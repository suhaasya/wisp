//! Virtualised data grid with staged edits (LUM-024 / LUM-028).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, uniform_list, App, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, UniformListScrollHandle, Window,
};
use wisp_core::{
    bridge::DbCommandPayload, build_commit_statements, build_table_select, sql_for_display,
    sql_for_execution, CellEdit, CommitStatement, EffectiveCell, FetchStrategy, GridBrowseSource,
    GridCommitSpec, GridStatus, MockTablePageSource, MysqlDialect, PagerConfig, PageSource,
    PostgresDialect, QueryableMockTableSource, QueryableSequentialSource, ResultPager, RowCount,
    RowKey, RowStage, SequentialPageSource, SessionError, SessionSqlPageSource, SortModel,
    StagedGridSnapshot, TableChangeSet, TableDataQuery, Viewport, ColumnMeta, ConnectionEngine,
    ConnectionId, Dialect,
};

use crate::bridge::{db_bridge, spawn_db, sql_fetch};
use crate::components::text_input::{TextInput, TextInputKind};
use crate::components::toast;

use super::cell::{format_cell_text, CellKind};
use super::columns::{ColumnLayout, ROW_NUMBER_WIDTH};
use super::copy::{CopyFormat, GridCopyJob};
use super::filter_bar::GridFilterBar;
use super::layout_cache::LayoutCache;
use super::row_detail::{
    detail_resize_handle, render_row_detail_panel, row_fields_from_pager, RowDetailState,
    FkNavigate, MAX_DETAIL_WIDTH, MIN_DETAIL_WIDTH,
};
use super::commit_review::{render_commit_review, CommitReviewState};
use super::edit::{effective_to_label, pk_column_names, stage_value};
use super::selection::{CellCoord, GridSelection};
use crate::workspace::WorkspaceView;
use super::status_global::grid_status;
use crate::theme::{self, ResolvedTheme};

const ROW_HEIGHT: f32 = 26.0;
const HEADER_HEIGHT: f32 = 34.0;

pub type GridConnectionMeta = (
    Option<ConnectionId>,
    SharedString,
    ConnectionEngine,
    bool,
    bool,
);

fn grid_connection_meta(
    conn: Option<(ConnectionId, SharedString, ConnectionEngine, bool, bool)>,
) -> GridConnectionMeta {
    conn.map_or(
        (
            None,
            SharedString::from("Not connected"),
            ConnectionEngine::PostgreSql,
            false,
            false,
        ),
        |(id, label, engine, production, safe)| {
            (Some(id), label, engine, production, safe)
        },
    )
}

pub struct DataGrid {
    focus_handle: FocusHandle,
    theme: ResolvedTheme,
    table_name: SharedString,
    query: TableDataQuery,
    filter_generation: u64,
    filter_bar: Entity<GridFilterBar>,
    sql_preview: SharedString,
    base_rows: u64,
    payload_cols: usize,
    pager: Rc<RefCell<ResultPager<GridBrowseSource>>>,
    workspace: gpui::WeakEntity<WorkspaceView>,
    shop_mock: bool,
    browse_columns: Vec<ColumnMeta>,
    pub detail: RowDetailState,
    layout: ColumnLayout,
    selection: GridSelection,
    scroll: UniformListScrollHandle,
    scroll_x: f32,
    resize: Option<(usize, f32)>,
    layout_cache: LayoutCache,
    copy_job: GridCopyJob,
    query_ms: u64,
    last_viewport: (u64, u32),
    pages_loaded: bool,
    read_only: bool,
    change_set: TableChangeSet,
    row_key_cache: HashMap<u64, RowKey>,
    edit_generation: u64,
    active_edit: Option<ActiveCellEdit>,
    connection_id: Option<ConnectionId>,
    connection_label: SharedString,
    production_env: bool,
    safe_mode: bool,
    engine: ConnectionEngine,
    pub(super) commit_review: CommitReviewState,
    commit_toast: Option<SharedString>,
    commit_highlight: Option<RowKey>,
    review_focus: FocusHandle,
}

#[derive(Clone)]
struct ActiveCellEdit {
    display_row: u64,
    col: usize,
    input: Entity<TextInput>,
}

enum DisplayRow {
    Pager(u64),
    Insert(usize),
}

fn is_shop_mock_table(name: &str) -> bool {
    matches!(
        name,
        "customers" | "orders" | "order_items" | "products" | "shipments" | "shipment_legs"
    )
}

fn build_browse_source(
    table: &str,
    query: &TableDataQuery,
    bench: bool,
    rows: u64,
    payload_cols: usize,
    mock_shop: bool,
    session: Option<(ConnectionId, ConnectionEngine, std::sync::Arc<wisp_core::DbBridge>, Vec<ColumnMeta>)>,
) -> (GridBrowseSource, ColumnLayout, u64, usize, bool) {
    if bench {
        let inner = SequentialPageSource::new(rows, payload_cols);
        let layout = ColumnLayout::from_meta(PageSource::columns(&inner));
        let source = GridBrowseSource::Sequential(QueryableSequentialSource::new(inner, query.clone()));
        return (source, layout, rows, payload_cols, false);
    }
    if let Some((connection_id, engine, bridge, columns)) = session {
        let dialect: &dyn Dialect = match engine {
            ConnectionEngine::PostgreSql => &PostgresDialect,
            ConnectionEngine::MySql | ConnectionEngine::MariaDb => &MysqlDialect,
        };
        let built = build_table_select(dialect, query, 300, 0);
        let sql = sql_for_display(&built, dialect);
        let fetcher = sql_fetch::shared_sql_fetcher(bridge, connection_id, sql);
        let source = GridBrowseSource::SessionSql(SessionSqlPageSource::new(
            columns,
            RowCount::Unknown,
            fetcher,
        ));
        let layout = ColumnLayout::from_meta(PageSource::columns(&source));
        return (source, layout, 0, 0, false);
    }
    if mock_shop && is_shop_mock_table(table) {
        let inner = MockTablePageSource::new(table, 50_000);
        let layout = ColumnLayout::from_meta(PageSource::columns(&inner));
        let source = GridBrowseSource::MockTable(QueryableMockTableSource::new(inner, query.clone()));
        return (source, layout, 50_000, 0, true);
    }
    let inner = SequentialPageSource::new(rows, payload_cols);
    let layout = ColumnLayout::from_meta(PageSource::columns(&inner));
    let source = GridBrowseSource::Sequential(QueryableSequentialSource::new(inner, query.clone()));
    (source, layout, rows, payload_cols, false)
}

impl DataGrid {
    pub fn new_table(
        name: impl Into<SharedString>,
        query: TableDataQuery,
        table_columns: Vec<ColumnMeta>,
        workspace: gpui::WeakEntity<WorkspaceView>,
        read_only: bool,
        connection: Option<(ConnectionId, SharedString, ConnectionEngine, bool, bool)>,
        mock_shop: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = name.into();
        Self::new_table_with_query(
            title,
            query,
            table_columns,
            workspace,
            read_only,
            connection,
            mock_shop,
            cx,
        )
    }

    pub fn new_table_with_query(
        name: impl Into<SharedString>,
        query: TableDataQuery,
        table_columns: Vec<ColumnMeta>,
        workspace: gpui::WeakEntity<WorkspaceView>,
        read_only: bool,
        connection: Option<(ConnectionId, SharedString, ConnectionEngine, bool, bool)>,
        mock_shop: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let theme = theme::read_global(cx).resolved().clone();
        let bench = std::env::var_os("WISP_BENCH_GRID").is_some();
        let (rows, payload_cols) = if bench {
            (1_000_000u64, 19usize)
        } else {
            (50_000, 9)
        };
        let table_name: SharedString = name.into();
        let session = connection.as_ref().and_then(|(id, _, engine, _, _)| {
            if mock_shop {
                return None;
            }
            let bridge = db_bridge(cx);
            Some((*id, *engine, bridge, table_columns.clone()))
        });
        let (source, layout, base_rows, payload_cols, shop_mock) = build_browse_source(
            &table_name,
            &query,
            bench,
            rows,
            payload_cols,
            mock_shop,
            session,
        );
        let pager = ResultPager::new(
            source.clone(),
            FetchStrategy::Offset,
            PagerConfig::default(),
        )
        .expect("pager");
        let pager = Rc::new(RefCell::new(pager));
        Self::from_pager_parts(
            table_name,
            query,
            workspace,
            read_only,
            grid_connection_meta(connection),
            source,
            layout,
            base_rows,
            payload_cols,
            shop_mock,
            table_columns,
            pager,
            cx,
        )
    }

    pub fn new_session_query(
        title: impl Into<SharedString>,
        columns: Vec<ColumnMeta>,
        pager: ResultPager<GridBrowseSource>,
        workspace: gpui::WeakEntity<WorkspaceView>,
        connection: Option<(ConnectionId, SharedString, ConnectionEngine, bool, bool)>,
        cx: &mut Context<Self>,
    ) -> Self {
        let table_name: SharedString = title.into();
        let query = TableDataQuery::for_table(table_name.to_string());
        let layout = ColumnLayout::from_meta(columns.clone());
        let pager = Rc::new(RefCell::new(pager));
        let dummy_query = query.clone();
        Self::from_pager_parts(
            table_name,
            query,
            workspace,
            true,
            grid_connection_meta(connection),
            GridBrowseSource::Sequential(QueryableSequentialSource::new(
                SequentialPageSource::new(0, 0),
                dummy_query,
            )),
            layout,
            0,
            0,
            false,
            columns,
            pager,
            cx,
        )
    }

    fn from_pager_parts(
        table_name: SharedString,
        query: TableDataQuery,
        workspace: gpui::WeakEntity<WorkspaceView>,
        read_only: bool,
        (connection_id, connection_label, engine, production_env, safe_mode): GridConnectionMeta,
        _source: GridBrowseSource,
        layout: ColumnLayout,
        base_rows: u64,
        payload_cols: usize,
        shop_mock: bool,
        browse_columns: Vec<ColumnMeta>,
        pager: Rc<RefCell<ResultPager<GridBrowseSource>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let theme = theme::read_global(cx).resolved().clone();
        let bench = std::env::var_os("WISP_BENCH_GRID").is_some();
        let pk_cols = pk_column_names(&layout);
        let change_set = TableChangeSet::with_pk_columns(pk_cols);
        let filter_bar = cx.new(GridFilterBar::new);
        filter_bar.update(cx, |bar, cx| bar.load_query(&query, cx));
        let sql_preview = SharedString::from(build_sql_preview(&query));
        let mut detail = RowDetailState::new_default();
        detail.width = super::row_detail_width(cx);
        let commit_review = CommitReviewState::new(cx, &theme);
        Self {
            focus_handle: cx.focus_handle(),
            theme,
            table_name,
            query,
            filter_generation: 1,
            filter_bar,
            sql_preview,
            base_rows,
            payload_cols,
            pager,
            layout,
            workspace,
            shop_mock,
            browse_columns,
            detail,
            selection: GridSelection::None,
            scroll: UniformListScrollHandle::default(),
            scroll_x: 0.0,
            resize: None,
            layout_cache: LayoutCache::default(),
            copy_job: GridCopyJob::default(),
            query_ms: if bench { 42 } else { 8 },
            last_viewport: (0, 40),
            pages_loaded: true,
            read_only,
            change_set,
            row_key_cache: HashMap::new(),
            edit_generation: 0,
            active_edit: None,
            connection_id,
            connection_label,
            production_env,
            safe_mode,
            engine,
            commit_review,
            commit_toast: None,
            commit_highlight: None,
            review_focus: cx.focus_handle(),
        }
    }

    fn dialect(&self) -> &'static dyn Dialect {
        match self.engine {
            ConnectionEngine::MySql => &MysqlDialect,
            _ => &PostgresDialect,
        }
    }

    fn pager_row_for_pk(&self, pk: &wisp_core::PrimaryKey) -> Option<u64> {
        let col = self.change_set.pk_columns.first()?;
        let value = pk.0.iter().find(|(c, _)| c == col).map(|(_, v)| v.as_str())?;
        value.parse::<u64>().ok()
    }

    fn collect_baselines(&mut self) -> HashMap<wisp_core::PrimaryKey, HashMap<String, String>> {
        let mut out = HashMap::new();
        for (key, edits) in self.change_set.updated_rows() {
            let RowKey::Pk(pk) = key else {
                continue;
            };
            let Some(row) = self.pager_row_for_pk(pk) else {
                continue;
            };
            let mut map = HashMap::new();
            for col in edits.keys() {
                let Some(ix) = self
                    .layout
                    .columns
                    .iter()
                    .position(|c| c.meta.name == *col)
                else {
                    continue;
                };
                if let Some(v) = self.base_cell_text(row, ix) {
                    map.insert(col.clone(), v);
                }
            }
            out.insert(pk.clone(), map);
        }
        out
    }

    fn build_commit_plans(&mut self) -> Vec<CommitStatement> {
        let spec = GridCommitSpec {
            schema: self.query.schema.clone(),
            table: self.query.table.clone(),
            columns: self
                .layout
                .columns
                .iter()
                .map(|c| c.meta.clone())
                .collect(),
            optimistic: true,
        };
        let baselines = self.collect_baselines();
        build_commit_statements(self.dialect(), &spec, &self.change_set, &baselines)
    }

    pub fn open_commit_review(&mut self, cx: &mut Context<Self>) {
        if self.change_set.staged_count() == 0 {
            return;
        }
        let plans = self.build_commit_plans();
        self.commit_review.sql_lines = sql_for_execution(&plans, self.dialect());
        self.commit_review.statements = plans;
        self.commit_review.connection_label = self.connection_label.clone();
        self.commit_review.environment_label = if self.production_env {
            "Production".into()
        } else {
            "Non-production".into()
        };
        self.commit_review.requires_delete_confirm =
            self.production_env && self.change_set.has_delete();
        self.commit_review.error = None;
        self.commit_review.open = true;
        cx.notify();
    }

    pub fn run_commit(&mut self, cx: &mut Context<Self>, direct: bool) {
        if self.change_set.staged_count() == 0 {
            return;
        }
        if !direct && !self.commit_review.open {
            self.open_commit_review(cx);
            return;
        }
        if self.production_env && self.change_set.has_delete() {
            let typed = self.commit_review.confirm_input.read(cx).content();
            if typed != self.query.table {
                self.commit_review.error = Some((
                    0,
                    "Table name confirmation does not match".into(),
                ));
                cx.notify();
                return;
            }
        }
        let Some(connection_id) = self.connection_id else {
            self.commit_toast = Some("Connect a database to commit".into());
            cx.notify();
            return;
        };
        let statements = if self.commit_review.sql_lines.is_empty() {
            sql_for_execution(&self.build_commit_plans(), self.dialect())
        } else {
            self.commit_review.sql_lines.clone()
        };
        self.commit_review.committing = true;
        let bridge = db_bridge(cx);
        spawn_db(
            cx,
            &bridge,
            DbCommandPayload::SessionCommitTransaction {
                id: connection_id,
                statements,
                write_approved: true,
            },
            |this, cx, result| {
                this.commit_review.committing = false;
                match result {
                    Ok(wisp_core::bridge::DbEventPayload::SessionCommitTransaction(Ok(outcome))) => {
                        this.change_set.discard_all();
                        this.row_key_cache.clear();
                        this.commit_review.open = false;
                        this.commit_highlight = None;
                        this.commit_toast = Some(format!(
                            "Committed {} statements · {} rows affected",
                            outcome.statements_run, outcome.rows_affected
                        ).into());
                        this.apply_filters(cx);
                    }
                    Ok(wisp_core::bridge::DbEventPayload::SessionCommitTransaction(Err(
                        SessionError::CommitFailed { index, message },
                    ))) => {
                        this.commit_review.error = Some((index, message));
                        if let Some(stmt) = this.commit_review.statements.get(index) {
                            this.commit_highlight = Some(stmt.row_key.clone());
                        }
                    }
                    Ok(_) | Err(_) => {
                        this.commit_toast =
                            Some("Commit failed — session unavailable".into());
                    }
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    fn editing_enabled(&self) -> bool {
        self.change_set.is_editable() && !self.read_only
    }

    fn edit_hint(&self) -> Option<String> {
        if self.read_only {
            return Some("Editing disabled (read-only connection)".into());
        }
        if !self.change_set.is_editable() {
            return Some("Editing disabled (no primary key)".into());
        }
        None
    }

    fn display_row_count(&self) -> u64 {
        self.pager.borrow().total_rows() + self.change_set.inserts().len() as u64
    }

    fn resolve_display_row(&self, display_row: u64) -> DisplayRow {
        let pager_rows = self.pager.borrow().total_rows();
        if display_row < pager_rows {
            DisplayRow::Pager(display_row)
        } else {
            DisplayRow::Insert((display_row - pager_rows) as usize)
        }
    }

    fn row_key_for_display(&mut self, display_row: u64) -> Option<RowKey> {
        match self.resolve_display_row(display_row) {
            DisplayRow::Insert(ix) => self
                .change_set
                .inserts()
                .get(ix)
                .map(|r| RowKey::Insert(r.temp_id)),
            DisplayRow::Pager(row) => {
                if let Some(key) = self.row_key_cache.get(&row) {
                    return Some(key.clone());
                }
                let mut parts = Vec::new();
                for name in &self.change_set.pk_columns {
                    let col_ix = self
                        .layout
                        .columns
                        .iter()
                        .position(|c| c.meta.name == *name)?;
                    let text = self.base_cell_text(row, col_ix)?;
                    parts.push((name.clone(), text));
                }
                let key = RowKey::Pk(wisp_core::PrimaryKey(parts));
                self.row_key_cache.insert(row, key.clone());
                Some(key)
            }
        }
    }

    pub fn staged_snapshot(&self) -> StagedGridSnapshot {
        self.change_set.to_staged_snapshot()
    }

    pub fn apply_staged_snapshot(&mut self, snap: StagedGridSnapshot) {
        self.change_set.apply_staged_snapshot(&snap);
        self.edit_generation = self.edit_generation.wrapping_add(1);
        self.layout_cache = LayoutCache::default();
    }

    fn bump_edits(&mut self, cx: &mut Context<Self>) {
        self.edit_generation = self.edit_generation.wrapping_add(1);
        self.layout_cache = LayoutCache::default();
        let workspace = self.workspace.clone();
        cx.defer(move |cx| {
            let _ = workspace.update(cx, |ws, cx| ws.touch_journal(cx));
        });
        cx.notify();
    }

    fn discard_all_edits(&mut self, cx: &mut Context<Self>) {
        self.change_set.discard_all();
        self.row_key_cache.clear();
        self.active_edit = None;
        self.bump_edits(cx);
    }

    fn base_cell_text(&self, row: u64, col: usize) -> Option<String> {
        let pager = self.pager.borrow();
        pager.cell_text(row, col).ok().flatten()
    }

    pub(super) fn stage_display_cell(
        &mut self,
        display_row: u64,
        col: usize,
        raw: &str,
        cx: &mut Context<Self>,
    ) {
        if !self.editing_enabled() {
            return;
        }
        let Some(key) = self.row_key_for_display(display_row) else {
            return;
        };
        let col_name = self.layout.columns[col].meta.name.clone();
        stage_value(&mut self.change_set, key, &col_name, raw);
        self.bump_edits(cx);
    }

    pub(super) fn set_cell_null(&mut self, display_row: u64, col: usize, cx: &mut Context<Self>) {
        if !self.editing_enabled() {
            return;
        }
        let Some(key) = self.row_key_for_display(display_row) else {
            return;
        };
        let col_name = self.layout.columns[col].meta.name.clone();
        self.change_set
            .stage_cell(key, col_name, CellEdit::Null);
        self.bump_edits(cx);
    }

    pub(super) fn set_cell_default(
        &mut self,
        display_row: u64,
        col: usize,
        cx: &mut Context<Self>,
    ) {
        if !self.editing_enabled() {
            return;
        }
        let Some(key) = self.row_key_for_display(display_row) else {
            return;
        };
        let col_name = self.layout.columns[col].meta.name.clone();
        self.change_set
            .stage_cell(key, col_name, CellEdit::Default);
        self.bump_edits(cx);
    }

    pub(super) fn revert_display_cell(
        &mut self,
        display_row: u64,
        col: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = self.row_key_for_display(display_row) else {
            return;
        };
        let col_name = self.layout.columns[col].meta.name.clone();
        self.change_set.revert_cell(&key, &col_name);
        self.bump_edits(cx);
    }

    fn delete_display_row(&mut self, display_row: u64, cx: &mut Context<Self>) {
        if !self.editing_enabled() {
            return;
        }
        let Some(key) = self.row_key_for_display(display_row) else {
            return;
        };
        self.change_set.delete_row(key);
        self.bump_edits(cx);
    }

    fn add_empty_row(&mut self, cx: &mut Context<Self>) {
        if !self.editing_enabled() {
            return;
        }
        self.change_set.insert_row(HashMap::new());
        self.bump_edits(cx);
    }

    fn duplicate_display_row(&mut self, display_row: u64, cx: &mut Context<Self>) {
        if !self.editing_enabled() {
            return;
        }
        let col_count = self.layout.columns.len();
        let mut seed = HashMap::new();
        for col_ix in 0..col_count {
            if self.layout.columns[col_ix].meta.is_pk {
                continue;
            }
            let name = self.layout.columns[col_ix].meta.name.clone();
            let (_, text) = self.cell_label(display_row, col_ix);
            if text.as_ref() != "NULL" {
                seed.insert(name, text.to_string());
            }
        }
        self.change_set.insert_row(seed);
        self.bump_edits(cx);
    }

    fn begin_cell_edit(&mut self, display_row: u64, col: usize, cx: &mut Context<Self>) {
        if !self.editing_enabled() {
            return;
        }
        if self.layout.columns[col].meta.is_pk {
            return;
        }
        let (_, current) = self.cell_label(display_row, col);
        let theme = self.theme.clone();
        let input = cx.new(|cx| {
            TextInput::new(cx, "", TextInputKind::Mono, theme)
        });
        input.update(cx, |field, cx| {
            field.set_content(current.as_ref());
            cx.notify();
        });
        self.active_edit = Some(ActiveCellEdit {
            display_row,
            col,
            input,
        });
        cx.notify();
    }

    fn commit_active_edit(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.active_edit.take() else {
            return;
        };
        let text = edit.input.read(cx).content().to_string();
        self.stage_display_cell(edit.display_row, edit.col, &text, cx);
    }

    fn cancel_active_edit(&mut self, cx: &mut Context<Self>) {
        if self.active_edit.take().is_some() {
            cx.notify();
        }
    }

    fn set_selection(&mut self, selection: GridSelection, cx: &mut Context<Self>) {
        if self.selection.primary_row() != selection.primary_row() {
            self.detail.clear_large_values();
        }
        self.selection = selection;
        cx.notify();
    }

    fn navigate_fk(&self, nav: FkNavigate, cx: &mut Context<Self>) {
        let query = wisp_core::query_for_fk_target(
            &nav.target_table,
            &nav.fk,
            &nav.key_values,
        );
        let title = nav.target_table.clone();
        self.workspace
            .update(cx, |ws, cx| ws.open_table_with_query(title, query, cx))
            .ok();
    }

    pub fn unload_for_background(&mut self) {
        if !self.pages_loaded {
            return;
        }
        self.pager.borrow_mut().unload_pages();
        self.layout_cache = LayoutCache::default();
        self.copy_job = GridCopyJob::default();
        self.pages_loaded = false;
        self.last_viewport = (0, 0);
    }

    pub fn mark_active(&mut self) {
        self.pages_loaded = true;
    }

    pub fn is_pages_loaded(&self) -> bool {
        self.pages_loaded
    }

    pub fn loaded_bytes(&self) -> usize {
        self.pager.borrow().loaded_bytes()
    }

    fn total_rows(&self) -> u64 {
        self.pager.borrow().total_rows()
    }

    fn sync_viewport(&mut self, first: u64, visible: u32) {
        if !self.pages_loaded {
            return;
        }
        if self.last_viewport == (first, visible) {
            return;
        }
        self.last_viewport = (first, visible);
        let _ = self.pager.borrow_mut().set_viewport(Viewport {
            first_row: first,
            visible_rows: visible,
        });
    }

    fn detail_fields_for_row(&mut self, display_row: u64) -> Vec<wisp_core::RowDetailField> {
        let col_count = self.layout.columns.len();
        let mut previews = Vec::with_capacity(col_count);
        for col_ix in 0..col_count {
            let meta = self.layout.columns[col_ix].meta.clone();
            let (_, text) = self.cell_label(display_row, col_ix);
            previews.push((meta.name, meta.type_name, text.to_string(), meta.is_pk));
        }
        row_fields_from_pager(previews)
    }

    fn cell_label(&mut self, display_row: u64, col: usize) -> (CellKind, SharedString) {
        let cache_key = display_row.wrapping_mul(10_000).wrapping_add(col as u64);
        if let Some(hit) = self.layout_cache.get(cache_key, col as u16) {
            return (CellKind::Text, hit);
        }
        let type_name = self.layout.columns[col].meta.type_name.clone();
        let col_name = self.layout.columns[col].meta.name.clone();
        let base = match self.resolve_display_row(display_row) {
            DisplayRow::Pager(row) => self.base_cell_text(row, col),
            DisplayRow::Insert(_) => None,
        };
        let effective = self
            .row_key_for_display(display_row)
            .map(|key| {
                self.change_set
                    .effective_cell(&key, &col_name, base.as_deref())
            })
            .unwrap_or_else(|| {
                base.map(EffectiveCell::Base)
                    .unwrap_or(EffectiveCell::Null)
            });
        let (kind, text) = effective_to_label(&effective, &type_name);
        let shared: SharedString = text.into();
        self.layout_cache.put(cache_key, col as u16, shared.clone());
        (kind, shared)
    }

    fn update_status(&self, cx: &App) {
        let (first, visible) = self.last_viewport;
        let last = first.saturating_add(visible as u64).saturating_sub(1);
        let count = self.pager.borrow().row_count();
        let (total, estimate) = match count {
            RowCount::Exact(n) => (Some(n), false),
            RowCount::Estimate { rows, .. } => (Some(rows), true),
            RowCount::Unknown => (None, false),
        };
        grid_status(cx).set(GridStatus {
            row_from: first,
            row_to: last.min(self.display_row_count().saturating_sub(1)),
            total_rows: total.map(|n| n + self.change_set.inserts().len() as u64),
            total_estimate: estimate,
            query_ms: self.query_ms,
            staged_changes: self.change_set.staged_count(),
            edit_hint: self.edit_hint(),
        });
    }

    fn session_for_browse(
        &self,
        cx: &Context<Self>,
    ) -> Option<(
        ConnectionId,
        ConnectionEngine,
        std::sync::Arc<wisp_core::DbBridge>,
        Vec<ColumnMeta>,
    )> {
        if self.shop_mock {
            return None;
        }
        let connection_id = self.connection_id?;
        let bridge = db_bridge(cx);
        Some((
            connection_id,
            self.engine,
            bridge,
            self.browse_columns.clone(),
        ))
    }

    fn apply_filters(&mut self, cx: &mut Context<Self>) {
        let mut next = self.filter_bar.read(cx).build_query(&self.table_name, cx);
        next.sort = self.query.sort.clone();
        self.query = next;
        self.filter_generation = self.filter_generation.wrapping_add(1);
        let bench = std::env::var_os("WISP_BENCH_GRID").is_some();
        let session = self.session_for_browse(cx);
        let (source, layout, base_rows, payload_cols, shop_mock) = build_browse_source(
            &self.table_name,
            &self.query,
            bench,
            self.base_rows,
            self.payload_cols,
            self.shop_mock,
            session,
        );
        self.base_rows = base_rows;
        self.payload_cols = payload_cols;
        self.shop_mock = shop_mock;
        self.layout = layout;
        if let Ok(pager) = ResultPager::new(
            source,
            FetchStrategy::Offset,
            PagerConfig::default(),
        ) {
            *self.pager.borrow_mut() = pager;
        }
        self.layout_cache = LayoutCache::default();
        self.row_key_cache.clear();
        self.last_viewport = (0, 0);
        self.sql_preview = SharedString::from(build_sql_preview(&self.query));
        self.query_ms = 12;
        cx.notify();
    }

    fn sort_by_column(&mut self, col_ix: usize, extend: bool, cx: &mut Context<Self>) {
        let name = self.layout.columns[col_ix].meta.name.clone();
        self.query.sort.toggle_column(name, extend);
        self.apply_filters(cx);
    }

    fn copy_selection(&mut self, cx: &mut Context<Self>, format: CopyFormat) {
        let rows: Vec<u64> = match &self.selection {
            GridSelection::None => return,
            GridSelection::Cell(c) => vec![c.row],
            GridSelection::Row(r) => vec![*r],
            GridSelection::Range { start, end } => (start.row..=end.row).collect(),
        };
        let cols = self.layout.visible_indices();
        let pager = Rc::clone(&self.pager);
        let layout = self.layout.clone();
        if rows.len() > 1_000 {
            self.copy_job.task = Some(cx.spawn(async move |this, cx| {
                let text = super::copy::collect_copy_text(pager, layout, &rows, &cols, format)
                    .unwrap_or_else(|err| format!("Copy failed: {err}"));
                this.update(cx, |_, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                })
                .ok();
            }));
        } else {
            let _ = super::copy::collect_copy_text_sync(pager, layout, &rows, &cols, format, cx);
        }
    }
}

impl Focusable for DataGrid {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DataGrid {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.update_status(cx);
        let theme = self.theme.clone();
        let c = theme.colors.clone();
        let total = self.display_row_count() as usize;
        let visible_cols = self.layout.visible_indices();
        let scroll_x = self.scroll_x;
        let selection = self.selection.clone();
        let scroll = self.scroll.clone();
        let layout = self.layout.clone();

        let sort = self.query.sort.clone();
        let theme_for_rows = theme.clone();
        let header = render_header(
            cx,
            &theme,
            &layout,
            &visible_cols,
            scroll_x,
            self.resize,
            &sort,
        );
        let sql_preview = self.sql_preview.clone();
        let filter_bar = self.filter_bar.clone();
        let row_ix = self.selection.primary_row();
        let detail_fields = row_ix
            .map(|r| self.detail_fields_for_row(r))
            .unwrap_or_default();
        let edit_hint = self
            .edit_hint()
            .unwrap_or_else(|| "Editing unavailable".into());
        let editing_enabled = self.editing_enabled();
        let staged_count = self.change_set.staged_count();
        div()
            .id("data-grid")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(c.panel.clone())
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.detail.panel_resize.take().is_some() {
                        super::set_row_detail_width(cx, this.detail.width);
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.detail.panel_resize.take().is_some() {
                        super::set_row_detail_width(cx, this.detail.width);
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if let Some(anchor) = this.detail.panel_resize {
                    this.detail.width = (anchor - f32::from(event.position.x))
                        .clamp(MIN_DETAIL_WIDTH, MAX_DETAIL_WIDTH);
                    cx.notify();
                }
            }))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key.as_str() == "space"
                    && !event.keystroke.modifiers.platform
                    && !event.keystroke.modifiers.control
                {
                    this.detail.open = !this.detail.open;
                    if !this.detail.open {
                        this.detail.clear_large_values();
                    }
                    cx.notify();
                    return;
                }
                if event.keystroke.key.as_str() == "enter" {
                    if this.active_edit.is_some() {
                        this.commit_active_edit(cx);
                    } else if let GridSelection::Cell(c) = &this.selection {
                        this.begin_cell_edit(c.row, c.col, cx);
                    }
                    return;
                }
                if event.keystroke.key.as_str() == "escape" {
                    this.cancel_active_edit(cx);
                    return;
                }
                if event.keystroke.modifiers.platform {
                    match event.keystroke.key.as_str() {
                        "s" if event.keystroke.modifiers.shift => {
                            if !this.production_env {
                                this.run_commit(cx, true);
                            }
                        }
                        "s" => this.open_commit_review(cx),
                        "c" => this.copy_selection(cx, CopyFormat::Tsv),
                        _ => {}
                    }
                }
            }))
            .child(filter_bar)
            .when(editing_enabled, |el| {
                el.child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_0p5()
                        .border_b_1()
                        .border_color(c.line2.clone())
                        .child(
                            div()
                                .id("grid-add-row")
                                .px_2()
                                .py_0p5()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .border_1()
                                .border_color(c.line.clone())
                                .on_click(cx.listener(|this, _, _, cx| this.add_empty_row(cx)))
                                .child("Add row"),
                        )
                        .child(
                            div()
                                .id("grid-dup-row")
                                .px_2()
                                .py_0p5()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .border_1()
                                .border_color(c.line.clone())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(r) = this.selection.primary_row() {
                                        this.duplicate_display_row(r, cx);
                                    }
                                }))
                                .child("Duplicate row"),
                        )
                        .child(
                            div()
                                .id("grid-del-row")
                                .px_2()
                                .py_0p5()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .border_1()
                                .border_color(c.line.clone())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(r) = this.selection.primary_row() {
                                        this.delete_display_row(r, cx);
                                    }
                                }))
                                .child("Delete row"),
                        )
                        .when(staged_count > 0, |bar| {
                            bar.child(
                                div()
                                    .id("grid-review-sql")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .text_xs()
                                    .cursor_pointer()
                                    .bg(c.accent_soft.clone())
                                    .text_color(c.ink1.clone())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_commit_review(cx);
                                    }))
                                    .child(format!("Review SQL ({staged_count})")),
                            )
                            .child(
                                div()
                                    .id("grid-discard-edits")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .text_xs()
                                    .cursor_pointer()
                                    .bg(c.staged.deleted.clone())
                                    .text_color(c.ink1.clone())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.discard_all_edits(cx);
                                    }))
                                    .child(format!("Discard all ({staged_count})")),
                            )
                        }),
                )
            })
            .when(!editing_enabled, |el| {
                el.child(
                    div()
                        .flex_none()
                        .px_2()
                        .py_0p5()
                        .border_b_1()
                        .border_color(c.line2.clone())
                        .text_xs()
                        .text_color(c.ink3.clone())
                        .child(edit_hint),
                )
            })
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_0p5()
                    .border_b_1()
                    .border_color(c.line2.clone())
                    .child(
                        div()
                            .id("filter-apply")
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .bg(c.accent.clone())
                            .text_color(c.on_accent.clone())
                            .on_click(cx.listener(|this, _, _, cx| this.apply_filters(cx)))
                            .child("Apply"),
                    )
                    .child(
                        div()
                            .id("filter-sql-preview")
                            .flex_1()
                            .text_xs()
                            .text_color(c.ink3.clone())
                            .font(theme.typography.gpui_mono_font())
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                    this.sql_preview.to_string(),
                                ));
                            }))
                            .child(format!("SQL: {sql_preview}")),
                    ),
            )
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                uniform_list(
                    "grid-rows",
                    total,
                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                        let visible = range.len().max(1) as u32;
                        let first = range.start as u64;
                        let pager_rows = this.pager.borrow().total_rows();
                        if first < pager_rows {
                            this.sync_viewport(first, visible);
                        } else if pager_rows > 0 {
                            this.sync_viewport(pager_rows.saturating_sub(1), visible);
                        }
                        let mut rows = Vec::new();
                        for row_ix in range {
                            let display_row = row_ix as u64;
                            rows.push(render_row(
                                cx,
                                &theme_for_rows,
                                display_row,
                                pager_rows,
                                &layout,
                                &visible_cols,
                                scroll_x,
                                &selection,
                                this,
                            ));
                        }
                        rows
                    }),
                )
                .flex_1()
                .track_scroll(&scroll)
                .on_scroll_wheel(cx.listener(|this, event: &gpui::ScrollWheelEvent, _, cx| {
                    if event.modifiers.shift {
                        let delta = event.delta.pixel_delta(gpui::Pixels::ZERO);
                        this.scroll_x = (this.scroll_x + f32::from(delta.x)).max(0.0);
                        cx.notify();
                    }
                })),
                            ),
                    )
                    .children(self.active_edit.as_ref().map(|edit| {
                        edit.input.clone()
                    }))
                    .when(self.detail.open, |row| {
                        let table = self.table_name.to_string();
                        let fields = detail_fields.clone();
                        let detail = self.detail.clone();
                        row.child(detail_resize_handle(cx))
                            .child(render_row_detail_panel(
                                cx,
                                &theme,
                                &table,
                                row_ix,
                                &fields,
                                &detail,
                                editing_enabled,
                                |this, nav, cx| this.navigate_fk(nav, cx),
                            ))
                    }),
            )
            .child(render_commit_review(
                cx,
                &theme,
                &self.commit_review,
                &self.review_focus,
            ))
            .children(self.commit_toast.as_ref().map(|msg| toast::toast(&theme, msg.clone())))
    }
}

fn build_sql_preview(query: &TableDataQuery) -> String {
    let built = build_table_select(&PostgresDialect, query, 300, 0);
    sql_for_display(&built, &PostgresDialect)
}

fn render_header(
    cx: &mut Context<DataGrid>,
    theme: &ResolvedTheme,
    layout: &ColumnLayout,
    visible_cols: &[usize],
    scroll_x: f32,
    resize: Option<(usize, f32)>,
    sort: &SortModel,
) -> impl IntoElement {
    let c = &theme.colors;
    let first_col = layout.first_visible_column(visible_cols, scroll_x);
    let mut x_off = 0.0f32;
    for &i in visible_cols {
        if i == first_col {
            break;
        }
        x_off += layout.columns[i].width;
    }
    let offset = scroll_x - x_off;

    div()
        .flex_none()
        .h(px(HEADER_HEIGHT))
        .flex()
        .border_b_1()
        .border_color(c.line.clone())
        .bg(c.sidebar.clone())
        .child(
            div()
                .w(px(ROW_NUMBER_WIDTH))
                .flex_none()
                .h_full()
                .border_r_1()
                .border_color(c.line2.clone())
                .bg(c.sidebar.clone()),
        )
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .when(offset > 0.0, |el| el.ml(px(-offset)))
                        .children(visible_cols.iter().filter(|&&i| i >= first_col).map(|&col_ix| {
                            let col = &layout.columns[col_ix];
                            let w = col.width;
                            let sort_mark = sort
                                .indicator(&col.meta.name)
                                .map(|ch| format!(" {ch}"))
                                .unwrap_or_default();
                            let label = format!("{}{}", col.meta.name, sort_mark);
                            div()
                                .id(("grid-hdr", col_ix as u32))
                                .w(px(w))
                                .flex_none()
                                .h_full()
                                .px_2()
                                .border_r_1()
                                .border_color(c.line2.clone())
                                .cursor_pointer()
                                .on_click(cx.listener({
                                    move |this, event: &gpui::ClickEvent, _, cx| {
                                        this.sort_by_column(col_ix, event.modifiers().shift, cx);
                                    }
                                }))
                                .on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                        this.layout.columns[col_ix].hidden = true;
                                        cx.notify();
                                    }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(c.ink2.clone())
                                        .child(label)
                                        .child(
                                            div()
                                                .text_color(c.ink3.clone())
                                                .font(theme.typography.gpui_mono_font())
                                                .child(col.meta.type_name.clone()),
                                        ),
                                )
                                .child(resize_handle(cx, col_ix, w, resize))
                        })),
                ),
        )
}

fn resize_handle(
    cx: &mut Context<DataGrid>,
    col_ix: usize,
    width: f32,
    _resize: Option<(usize, f32)>,
) -> impl IntoElement {
    div()
        .absolute()
        .right_0()
        .top_0()
        .h_full()
        .w(px(4.0))
        .cursor_col_resize()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _, _| {
                this.resize = Some((col_ix, f32::from(event.position.x) - width));
            }),
        )
}

fn render_row(
    cx: &mut Context<DataGrid>,
    theme: &ResolvedTheme,
    display_row: u64,
    pager_rows: u64,
    layout: &ColumnLayout,
    visible_cols: &[usize],
    scroll_x: f32,
    selection: &GridSelection,
    grid: &mut DataGrid,
) -> impl IntoElement {
    let c = &theme.colors;
    let row_key = grid.row_key_for_display(display_row);
    let highlight = grid
        .commit_highlight
        .as_ref()
        .zip(row_key.as_ref())
        .is_some_and(|(a, b)| a == b);
    let stage = if highlight {
        RowStage::Deleted
    } else {
        row_key
            .as_ref()
            .map(|k| grid.change_set.row_state(k))
            .unwrap_or(if display_row >= pager_rows {
                RowStage::Inserted
            } else {
                RowStage::Unchanged
            })
    };
    let deleted = matches!(stage, RowStage::Deleted);
    let selected_row = matches!(selection, GridSelection::Row(r) if *r == display_row)
        || matches!(selection, GridSelection::Range { start, end } if display_row >= start.row && display_row <= end.row);
    let row_bg = match stage {
        RowStage::Deleted => c.staged.deleted.clone(),
        RowStage::Inserted => c.staged.inserted.clone(),
        RowStage::Modified => c.staged.modified.clone(),
        RowStage::Unchanged if selected_row => c.accent_soft.clone(),
        RowStage::Unchanged => c.panel.clone(),
    };
    let first_col = layout.first_visible_column(visible_cols, scroll_x);
    let mut x_off = 0.0f32;
    for &i in visible_cols {
        if i == first_col {
            break;
        }
        x_off += layout.columns[i].width;
    }
    let offset = scroll_x - x_off;

    div()
        .id(("grid-row", display_row as u32))
        .h(px(ROW_HEIGHT))
        .flex()
        .w_full()
        .bg(row_bg)
        .child(
            div()
                .id(("grid-rn", display_row as u32))
                .w(px(ROW_NUMBER_WIDTH))
                .flex_none()
                .h_full()
                .flex()
                .items_center()
                .justify_end()
                .pr_1()
                .border_r_1()
                .border_color(c.line2.clone())
                .bg(c.sidebar.clone())
                .text_xs()
                .text_color(c.ink3.clone())
                .font(theme.typography.gpui_mono_font())
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_selection(GridSelection::Row(display_row), cx);
                }))
                .child(format!("{}", display_row + 1)),
        )
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .child(
                    div()
                        .flex()
                        .when(offset > 0.0, |el| el.ml(px(-offset)))
                        .children(visible_cols.iter().filter(|&&i| i >= first_col).map(|&col_ix| {
                            let w = layout.columns[col_ix].width;
                            let selected = selection.contains(display_row, col_ix);
                            let (kind, label) = grid.cell_label(display_row, col_ix);
                            div()
                                            .id(("grid-cell", display_row as u32 * 1000 + col_ix as u32))
                                .w(px(w))
                                .flex_none()
                                .h_full()
                                .flex()
                                .items_center()
                                .px_2()
                                .border_r_1()
                                .border_color(c.line2.clone())
                                .text_xs()
                                .font(theme.typography.gpui_mono_font())
                                .text_color(match kind {
                                    CellKind::Null => c.syntax.null,
                                    CellKind::Number => c.syntax.number,
                                    _ => c.ink1,
                                })
                                .when(kind == CellKind::Number, |el| el.justify_end())
                                .when(kind == CellKind::Null, |el| el.italic())
                                .bg(if selected {
                                    c.accent_soft.clone()
                                } else {
                                    row_bg.clone()
                                })
                                .cursor_pointer()
                                .when(deleted, |el| el.line_through())
                                .on_click(cx.listener({
                                    let display_row = display_row;
                                    move |this, event: &gpui::ClickEvent, _, cx| {
                                    let coord = CellCoord { row: display_row, col: col_ix };
                                    let next = if event.modifiers().shift {
                                        match &this.selection {
                                            GridSelection::Cell(start) | GridSelection::Range { start, .. } => {
                                                GridSelection::normalize_range(*start, coord)
                                            }
                                            GridSelection::Row(r) => GridSelection::normalize_range(
                                                CellCoord { row: *r, col: 0 },
                                                coord,
                                            ),
                                            GridSelection::None => GridSelection::Cell(coord),
                                        }
                                    } else {
                                        GridSelection::Cell(coord)
                                    };
                                    this.set_selection(next, cx);
                                    if event.click_count() > 1 && this.editing_enabled() {
                                        this.begin_cell_edit(display_row, col_ix, cx);
                                    }
                                }}))
                                .child(label)
                        })),
                ),
        )
}
