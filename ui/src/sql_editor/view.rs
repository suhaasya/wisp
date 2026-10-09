//! Multiline SQL editor view (LUM-030).

use std::ops::Range;

use gpui::{
    actions, div, prelude::*, px, App, Bounds, ClipboardItem, Context, CursorStyle, Element,
    ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable,
    GlobalElementId, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point,
    Render, SharedString, StatefulInteractiveElement, Styled, UTF16Selection, Window,
};

use wisp_core::{
    complete_at_cursor, search_history_file, spans_for_line, CompletionContext, CompletionItem,
    ConnectionEngine, DbEventPayload, HighlightSpan, QueryHistoryEntry, RecentCompletionUse,
    Selection, SharedMetadataCache, Snippet, SplitFlavor, SqlDialect, SqlEditor,
    SqlSyntaxHighlighter, SyntaxTokenKind, WispError, WispPaths,
};

use crate::workspace::WorkspaceView;

use super::history::record_run_history;
use super::query_run::{
    dispatch_run_script, plan_statements, request_cancel, QueryRunState, ResultPaneTab, RunScope,
};

use crate::bridge::db_bridge;
use crate::components::focus::focus_visible_ring;
use crate::components::text_input::{TextInput, TextInputKind};
use crate::theme::{ResolvedTheme, SyntaxColors};

use super::utf16::{range_from_utf16, range_to_utf16};

actions!(
    wisp_sql_editor,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Undo,
        Redo,
        ToggleSoftWrap,
        ToggleFind,
        FindNext,
        NewLine,
        TabInsert,
        AcceptCompletion,
        DismissCompletion,
        CompletionUp,
        CompletionDown,
        RunCurrent,
        RunAll,
        CancelQueryRun,
        ConfirmWriteRun,
        ToggleHistoryDrawer,
        SaveSnippet,
    ]
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HistoryDrawerTab {
    History,
    Snippets,
}

const GUTTER_CHARS: usize = 5;
const CHAR_WIDTH_PX: f32 = 7.5;

pub fn split_flavor_for(engine: ConnectionEngine) -> SplitFlavor {
    match engine {
        ConnectionEngine::MySql | ConnectionEngine::MariaDb => SplitFlavor::Mysql,
        _ => SplitFlavor::Standard,
    }
}

pub struct SqlEditorView {
    focus_handle: FocusHandle,
    editor: SqlEditor,
    theme: ResolvedTheme,
    split_flavor: SplitFlavor,
    find_open: bool,
    find_input: Entity<TextInput>,
    replace_input: Entity<TextInput>,
    last_bounds: Option<Bounds<Pixels>>,
    line_height: Pixels,
    highlighter: SqlSyntaxHighlighter,
    highlight_source: String,
    highlight_spans: Vec<HighlightSpan>,
    metadata_cache: SharedMetadataCache,
    recent_completions: RecentCompletionUse,
    completion_ctx: Option<CompletionContext>,
    completion_items: Vec<CompletionItem>,
    completion_index: usize,
    completion_open: bool,
    workspace: gpui::WeakEntity<WorkspaceView>,
    run: QueryRunState,
    history_open: bool,
    history_tab: HistoryDrawerTab,
    history_search: Entity<TextInput>,
    history_entries: Vec<QueryHistoryEntry>,
    connection_snippets: Vec<Snippet>,
    save_snippet_open: bool,
    snippet_name_input: Entity<TextInput>,
}

impl SqlEditorView {
    pub fn new(
        theme: ResolvedTheme,
        split_flavor: SplitFlavor,
        metadata_cache: SharedMetadataCache,
        workspace: gpui::WeakEntity<WorkspaceView>,
        cx: &mut Context<Self>,
    ) -> Self {
        let find_input = cx.new(|cx| {
            TextInput::new(cx, "Find…", TextInputKind::Mono, theme.clone())
        });
        let replace_input = cx.new(|cx| {
            TextInput::new(cx, "Replace…", TextInputKind::Mono, theme.clone())
        });
        let history_search = cx.new(|cx| {
            TextInput::new(cx, "Search history…", TextInputKind::Mono, theme.clone())
        });
        let snippet_name_input = cx.new(|cx| {
            TextInput::new(cx, "Snippet name…", TextInputKind::Mono, theme.clone())
        });
        let dialect = SqlDialect::from_split_flavor(split_flavor);
        Self {
            focus_handle: cx.focus_handle(),
            editor: SqlEditor::new(""),
            theme,
            split_flavor,
            find_open: false,
            find_input,
            replace_input,
            last_bounds: None,
            line_height: px(18.),
            highlighter: SqlSyntaxHighlighter::new(dialect),
            highlight_source: String::new(),
            highlight_spans: Vec::new(),
            metadata_cache,
            recent_completions: RecentCompletionUse::default(),
            completion_ctx: None,
            completion_items: Vec::new(),
            completion_index: 0,
            completion_open: false,
            workspace,
            run: QueryRunState::default(),
            history_open: false,
            history_tab: HistoryDrawerTab::History,
            history_search,
            history_entries: Vec::new(),
            connection_snippets: Vec::new(),
            save_snippet_open: false,
            snippet_name_input,
        }
    }

    fn refresh_history_panel(&mut self, cx: &mut Context<Self>) {
        let Some(connection_id) = self
            .workspace
            .update(cx, |ws, _| ws.connection_id())
            .ok()
            .flatten()
        else {
            self.history_entries.clear();
            self.connection_snippets.clear();
            return;
        };
        let query = self.history_search.read(cx).content().to_string();
        let path = WispPaths::resolve().query_history_jsonl(connection_id);
        self.history_entries =
            search_history_file(&path, &query, 100).unwrap_or_default();
        self.connection_snippets = self
            .workspace
            .update(cx, |ws, _| ws.snippets_for_connection(connection_id))
            .unwrap_or_default();
    }

    fn insert_sql(&mut self, sql: &str, cx: &mut Context<Self>) {
        let range = self.editor.primary_selection().range();
        if range.is_empty() {
            self.editor.insert(sql);
        } else {
            self.editor.replace_primary(range, sql);
        }
        self.highlight_source.clear();
        cx.notify();
    }

    fn toggle_history_drawer(&mut self, cx: &mut Context<Self>) {
        self.history_open = !self.history_open;
        if self.history_open {
            self.refresh_history_panel(cx);
        }
        cx.notify();
    }

    fn save_snippet_from_editor(&mut self, cx: &mut Context<Self>) {
        let Some(connection_id) = self
            .workspace
            .update(cx, |ws, _| ws.connection_id())
            .ok()
            .flatten()
        else {
            return;
        };
        let name = self.snippet_name_input.read(cx).content().trim().to_string();
        if name.is_empty() {
            self.save_snippet_open = true;
            cx.notify();
            return;
        }
        let sql = if self.editor.primary_selection().is_caret() {
            self.editor.text()
        } else {
            let range = self.editor.primary_selection().range();
            let full = self.editor.text();
            full.chars()
                .skip(range.start)
                .take(range.end.saturating_sub(range.start))
                .collect()
        };
        let _ = self.workspace.update(cx, |ws, _| {
            ws.save_snippet(connection_id, name, sql, Vec::new())
        });
        self.save_snippet_open = false;
        self.refresh_history_panel(cx);
        cx.notify();
    }

    fn start_run(&mut self, scope: RunScope, cx: &mut Context<Self>) {
        if self.run.running {
            return;
        }
        let Some(connection_id) = self
            .workspace
            .update(cx, |ws, _| ws.connection_id())
            .ok()
            .flatten()
        else {
            self.run.message_lines.push("Connect to a database first.".into());
            cx.notify();
            return;
        };
        let text = self.editor.text();
        let selection = self.editor.primary_selection();
        let selection_bytes = if selection.is_caret() {
            None
        } else {
            Some(self.editor.primary_selection_bytes())
        };
        let statements = plan_statements(
            &text,
            scope,
            self.split_flavor,
            selection_bytes,
            self.editor.caret_byte(),
        );
        if statements.is_empty() {
            return;
        }
        self.run.error_byte_range = None;
        self.run.running = true;
        self.run.started_at = Some(std::time::Instant::now());
        self.run.last_run_statements = statements.clone();
        self.run.last_run_started = Some(std::time::Instant::now());
        let write_approved = self.run.write_approved;
        let history_enabled = self
            .workspace
            .update(cx, |ws, _| ws.query_history_enabled(connection_id))
            .unwrap_or(true);
        let workspace = self.workspace.clone();
        let entity = cx.entity().downgrade();
        let cancel = dispatch_run_script(cx, connection_id, statements, write_approved, {
            move |this, cx, result| {
                this.run.running = false;
                this.run.started_at = None;
                this.run.cancel = None;
                this.run.write_approved = false;
                let started = this.run.last_run_started.take().unwrap_or_else(std::time::Instant::now);
                match result {
                    Ok(DbEventPayload::SessionRunScript(Ok(report))) => {
                        this.run.apply_report(report.outcomes.clone(), report.cancelled);
                        record_run_history(
                            connection_id,
                            history_enabled,
                            &this.run.last_run_statements,
                            &report.outcomes,
                            report.cancelled,
                            started,
                        );
                    }
                    Ok(DbEventPayload::SessionRunScript(Err(
                        wisp_core::SessionError::WriteNeedsConfirmation { sql },
                    ))) => {
                        this.run.pending_write_confirm = Some(sql);
                    }
                    Ok(DbEventPayload::SessionRunScript(Err(err))) => {
                        this.run.message_lines.push(format!("ERROR: {err}"));
                    }
                    Ok(_) => {}
                    Err(err) if err.to_string().contains("cancelled") => {
                        this.run.message_lines.push("Query cancelled.".into());
                    }
                    Err(_) => {
                        this.run.message_lines.push("Query run failed.".into());
                    }
                }
                let _ = workspace;
                let _ = entity;
                cx.notify();
            }
        });
        self.run.cancel = Some(cancel);
        cx.notify();
        self.schedule_run_timer(cx);
    }

    fn schedule_run_timer(&mut self, cx: &mut Context<Self>) {
        if !self.run.running {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            this.update(cx, |view, cx| {
                if view.run.running {
                    view.schedule_run_timer(cx);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn cancel_run(&mut self, cx: &mut Context<Self>) {
        if let Some(token) = self.run.cancel.take() {
            token.cancel();
        }
        if let Some(id) = self
            .workspace
            .update(cx, |ws, _| ws.connection_id())
            .ok()
            .flatten()
        {
            request_cancel(id, cx);
        }
        self.run.running = false;
        self.run.started_at = None;
        cx.notify();
    }

    fn confirm_write_run(&mut self, cx: &mut Context<Self>) {
        self.run.write_approved = true;
        self.run.pending_write_confirm = None;
        self.start_run(RunScope::CurrentOrSelection, cx);
    }

    pub fn set_split_flavor(&mut self, flavor: SplitFlavor) {
        self.split_flavor = flavor;
        self.highlighter
            .set_dialect(SqlDialect::from_split_flavor(flavor));
        self.highlight_source.clear();
    }

    fn refresh_highlights(&mut self) {
        let text = self.editor.text();
        if text == self.highlight_source {
            return;
        }
        self.highlight_spans = self.highlighter.highlight(&text);
        self.highlight_source = text;
    }

    fn refresh_completions(&mut self, cx: &mut Context<Self>) {
        let text = self.editor.text();
        let byte = self.editor.caret_byte();
        let mut cache = self.metadata_cache.borrow_mut();
        let schema = cache.default_schema().to_string();
        let snippets = self
            .workspace
            .update(cx, |ws, _| {
                ws.connection_id()
                    .map(|id| ws.snippet_suggestions(id))
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let (ctx, items) = complete_at_cursor(
            &text,
            byte,
            self.split_flavor,
            &schema,
            &mut cache,
            &self.recent_completions,
            &snippets,
        );
        self.completion_ctx = ctx;
        self.completion_items = items;
        self.completion_open = !self.completion_items.is_empty();
        self.completion_index = 0;
    }

    fn accept_completion(&mut self, cx: &mut Context<Self>) {
        if !self.completion_open {
            return;
        }
        let Some(ctx) = self.completion_ctx.clone() else {
            return;
        };
        let Some(item) = self.completion_items.get(self.completion_index).cloned() else {
            return;
        };
        self.editor.replace_byte_range(
            ctx.replace_start_byte..ctx.replace_end_byte,
            item.text_to_insert(),
        );
        self.recent_completions.bump(&item.label);
        self.completion_open = false;
        self.highlight_source.clear();
        cx.notify();
    }

    fn dismiss_completion(&mut self, cx: &mut Context<Self>) {
        if self.run.running {
            self.cancel_run(cx);
            return;
        }
        if self.completion_open {
            self.completion_open = false;
            cx.notify();
        }
    }

    fn run_current(&mut self, _: &RunCurrent, _: &mut Window, cx: &mut Context<Self>) {
        self.start_run(RunScope::CurrentOrSelection, cx);
    }

    fn run_all(&mut self, _: &RunAll, _: &mut Window, cx: &mut Context<Self>) {
        self.start_run(RunScope::All, cx);
    }

    fn cancel_query_run(&mut self, _: &CancelQueryRun, _: &mut Window, cx: &mut Context<Self>) {
        self.cancel_run(cx);
    }

    fn confirm_write(&mut self, _: &ConfirmWriteRun, _: &mut Window, cx: &mut Context<Self>) {
        self.confirm_write_run(cx);
    }

    fn toggle_history(&mut self, _: &ToggleHistoryDrawer, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_history_drawer(cx);
    }

    fn save_snippet_action(&mut self, _: &SaveSnippet, _: &mut Window, cx: &mut Context<Self>) {
        if self.save_snippet_open {
            self.save_snippet_from_editor(cx);
        } else {
            self.save_snippet_open = true;
            cx.notify();
        }
    }

    pub fn editor_text(&self) -> String {
        self.editor.text()
    }

    pub fn set_editor_text(&mut self, text: String, cx: &mut Context<Self>, touch_journal: bool) {
        self.editor.set_text(&text);
        self.highlight_source.clear();
        self.refresh_highlights();
        if touch_journal {
            self.notify_journal(cx);
        }
        cx.notify();
    }

    fn notify_journal(&self, cx: &mut Context<Self>) {
        let workspace = self.workspace.clone();
        cx.defer(move |cx| {
            let _ = workspace.update(cx, |ws, cx| ws.touch_journal(cx));
        });
    }

    pub fn set_theme(&mut self, theme: ResolvedTheme, cx: &mut Context<Self>) {
        self.theme = theme.clone();
        self.find_input
            .update(cx, |input, _| input.set_theme(theme.clone()));
        self.replace_input.update(cx, |input, _| input.set_theme(theme));
    }

    fn primary_char_range(&self) -> Range<usize> {
        self.editor.primary_selection().range()
    }

    fn insert_or_replace(&mut self, text: &str, cx: &mut Context<Self>) {
        let range = self.primary_char_range();
        if range.is_empty() {
            self.editor.insert(text);
        } else {
            self.editor.replace_primary(range, text);
        }
        self.highlight_source.clear();
        self.refresh_completions(cx);
        self.notify_journal(cx);
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.primary_char_range();
        if range.is_empty() {
            self.editor.delete_backward();
        } else {
            self.editor.replace_primary(range, "");
        }
        self.highlight_source.clear();
        self.refresh_completions(cx);
        self.notify_journal(cx);
        cx.notify();
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.primary_char_range();
        if range.is_empty() {
            self.editor.delete_forward();
        } else {
            self.editor.replace_primary(range, "");
        }
        self.highlight_source.clear();
        self.refresh_completions(cx);
        self.notify_journal(cx);
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_left(false);
        cx.notify();
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_right(false);
        cx.notify();
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_line_start();
        cx.notify();
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_line_end();
        cx.notify();
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.select_all();
        cx.notify();
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.insert_or_replace(&text, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.primary_char_range();
        if !range.is_empty() {
            let text = self.editor.text();
            let slice: String = text.chars().skip(range.start).take(range.end - range.start).collect();
            cx.write_to_clipboard(ClipboardItem::new_string(slice));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        self.copy(&Copy, window, cx);
        let range = self.primary_char_range();
        if !range.is_empty() {
            self.editor.replace_primary(range, "");
            self.notify_journal(cx);
            cx.notify();
        }
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.undo();
        self.notify_journal(cx);
        cx.notify();
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.redo();
        self.notify_journal(cx);
        cx.notify();
    }

    fn toggle_soft_wrap(&mut self, _: &ToggleSoftWrap, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.set_soft_wrap(!self.editor.soft_wrap());
        cx.notify();
    }

    fn toggle_find(&mut self, _: &ToggleFind, _: &mut Window, cx: &mut Context<Self>) {
        self.find_open = !self.find_open;
        cx.notify();
    }

    fn find_next(&mut self, _: &FindNext, _: &mut Window, cx: &mut Context<Self>) {
        let needle = self.find_input.read(cx).content().to_string();
        if needle.is_empty() {
            return;
        }
        let from = self.editor.primary_selection().head;
        if let Some(range) = self.editor.find_next(&needle, from, true) {
            self.editor.set_primary_selection(Selection {
                anchor: range.start,
                head: range.end,
            });
            cx.notify();
        }
    }

    fn new_line(&mut self, _: &NewLine, _: &mut Window, cx: &mut Context<Self>) {
        if self.completion_open {
            self.accept_completion(cx);
            return;
        }
        self.insert_or_replace("\n", cx);
    }

    fn tab_insert(&mut self, _: &TabInsert, _: &mut Window, cx: &mut Context<Self>) {
        if self.completion_open {
            self.accept_completion(cx);
            return;
        }
        self.insert_or_replace("    ", cx);
    }

    fn accept_completion_action(&mut self, _: &AcceptCompletion, _: &mut Window, cx: &mut Context<Self>) {
        self.accept_completion(cx);
    }

    fn dismiss_completion_action(
        &mut self,
        _: &DismissCompletion,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_completion(cx);
    }

    fn completion_up(&mut self, _: &CompletionUp, _: &mut Window, cx: &mut Context<Self>) {
        if self.completion_open && self.completion_index > 0 {
            self.completion_index -= 1;
            cx.notify();
        }
    }

    fn completion_down(&mut self, _: &CompletionDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.completion_open && self.completion_index + 1 < self.completion_items.len() {
            self.completion_index += 1;
            cx.notify();
        }
    }

    fn char_index_for_point(&self, point: Point<Pixels>, bounds: Bounds<Pixels>) -> usize {
        let inner_left = bounds.left() + px(GUTTER_CHARS as f32 * CHAR_WIDTH_PX + 8.);
        let y = point.y - bounds.top();
        let line_ix = (y / self.line_height).floor().max(0.) as usize;
        let line_ix = line_ix.min(self.editor.line_count().saturating_sub(1));
        let col = ((point.x - inner_left) / px(CHAR_WIDTH_PX))
            .floor()
            .max(0.) as usize;
        let line_text = self.editor.line_text(line_ix);
        let col = col.min(line_text.chars().count());
        self.editor.line_start_char(line_ix) + col
    }

    fn current_statement_preview(&self) -> SharedString {
        let text = self.editor.text();
        let range = self.editor.current_statement_bytes(self.split_flavor);
        let end = range.end.min(text.len());
        let slice = &text[range.start..end];
        let preview: String = slice.chars().take(120).collect();
        if slice.chars().count() > 120 {
            format!("{preview}…")
        } else {
            preview
        }
        .into()
    }
}

impl EntityInputHandler for SqlEditorView {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.editor.text();
        let range = range_from_utf16(&text, &range_utf16);
        actual_range.replace(range_to_utf16(&text, &range));
        Some(
            text.chars()
                .skip(range.start)
                .take(range.end.saturating_sub(range.start))
                .collect(),
        )
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let text = self.editor.text();
        let range = self.primary_char_range();
        Some(UTF16Selection {
            range: range_to_utf16(&text, &range),
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.editor.text();
        let range = range_utf16
            .as_ref()
            .map(|r| range_from_utf16(&text, r))
            .unwrap_or_else(|| self.primary_char_range());
        self.editor.replace_primary(range, new_text);
        self.highlight_source.clear();
        self.refresh_completions(cx);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text_in_range(range_utf16, new_text, window, cx);
        if let Some(sel) = new_selected_range_utf16 {
            let text = self.editor.text();
            let range = range_from_utf16(&text, &sel);
            self.editor.set_primary_selection(Selection {
                anchor: range.start,
                head: range.end,
            });
            cx.notify();
        }
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(bounds)
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let char_ix = self.char_index_for_point(point, bounds);
        let text = self.editor.text();
        Some(range_to_utf16(&text, &(char_ix..char_ix)).start)
    }
}

struct SqlEditorElement {
    view: Entity<SqlEditorView>,
}

impl IntoElement for SqlEditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SqlEditorElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let view = self.view.read(cx);
        let lines = view.editor.line_count().max(1);
        let mut style = gpui::Style::default();
        style.size.width = gpui::relative(1.).into();
        style.size.height = (view.line_height * lines as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Window,
        _: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let view = self.view.read(cx);
        let focus = view.focus_handle.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        self.view.update(cx, |view, _| {
            view.last_bounds = Some(bounds);
        });
        let _ = window;
    }
}

impl Focusable for SqlEditorView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SqlEditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_highlights();
        self.refresh_completions(cx);
        let mono = self.theme.typography.gpui_mono_font();
        let line_h = window.line_height();
        self.line_height = line_h;
        let primary_line = self.editor.char_to_line(self.editor.primary_selection().head);
        let error_line = self
            .run
            .error_byte_range
            .as_ref()
            .map(|r| self.editor.byte_to_line(r.start));
        let run_elapsed = self.run.elapsed_ms();
        let run_running = self.run.running;
        let run_messages = self.run.message_lines.clone();
        let result_tabs = self.run.result_tabs.clone();
        let active_result = self.run.active_result_tab;
        let pending_write = self.run.pending_write_confirm.clone();
        let bridge = db_bridge(cx);
        let connection_id = self
            .workspace
            .update(cx, |ws, _| ws.connection_id())
            .ok()
            .flatten();
        let highlight_spans = self.highlight_spans.clone();
        let line_count = self.editor.line_count();

        let gutter_w = px(GUTTER_CHARS as f32 * CHAR_WIDTH_PX + 8.);
        let find_open = self.find_open;
        let find_input = self.find_input.clone();
        let replace_input = self.replace_input.clone();
        let soft_wrap = self.editor.soft_wrap();
        let stmt_preview = self.current_statement_preview();
        let c = &self.theme.colors;
        let syntax = c.syntax;
        let ink1 = c.ink1;
        let line_rows: Vec<(String, Vec<(SyntaxTokenKind, String)>)> = (0..line_count)
            .map(|line| {
                let line_text = self.editor.line_text(line);
                let line_start = self.editor.line_start_byte(line);
                let segs = spans_for_line(&line_text, line_start, &highlight_spans);
                (line_text, segs)
            })
            .collect();
        let completion_open = self.completion_open;
        let completion_index = self.completion_index;
        let completion_items = self.completion_items.clone();
        let history_open = self.history_open;
        let history_tab = self.history_tab;
        let history_entries = self.history_entries.clone();
        let connection_snippets = self.connection_snippets.clone();
        let history_search = self.history_search.clone();
        let snippet_name_input = self.snippet_name_input.clone();
        let save_snippet_open = self.save_snippet_open;

        div()
            .id("wisp-sql-editor")
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .key_context("WispSqlEditor")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .tab_index(0)
            .focus_visible(focus_visible_ring(c))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::toggle_soft_wrap))
            .on_action(cx.listener(Self::toggle_find))
            .on_action(cx.listener(Self::find_next))
            .on_action(cx.listener(Self::new_line))
            .on_action(cx.listener(Self::tab_insert))
            .on_action(cx.listener(Self::accept_completion_action))
            .on_action(cx.listener(Self::dismiss_completion_action))
            .on_action(cx.listener(Self::completion_up))
            .on_action(cx.listener(Self::completion_down))
            .on_action(cx.listener(Self::run_current))
            .on_action(cx.listener(Self::run_all))
            .on_action(cx.listener(Self::cancel_query_run))
            .on_action(cx.listener(Self::confirm_write))
            .on_action(cx.listener(Self::toggle_history))
            .on_action(cx.listener(Self::save_snippet_action))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(c.line)
                    .text_xs()
                    .text_color(c.ink2)
                    .child(
                        div()
                            .id("sql-run-current")
                            .cursor_pointer()
                            .text_color(c.ink1)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.start_run(RunScope::CurrentOrSelection, cx);
                            }))
                            .child("Run (⌘↩)"),
                    )
                    .child(
                        div()
                            .id("sql-run-all")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.start_run(RunScope::All, cx);
                            }))
                            .child("Run all"),
                    )
                    .child(
                        div()
                            .id("sql-history-toggle")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_history_drawer(cx)))
                            .child("History"),
                    )
                    .child(
                        div()
                            .id("sql-save-snippet")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.save_snippet_open = true;
                                cx.notify();
                            }))
                            .child("Save snippet"),
                    )
                    .child(if run_running {
                        div()
                            .id("sql-cancel-run")
                            .cursor_pointer()
                            .text_color(c.accent)
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_run(cx)))
                            .child(format!("Cancel · {run_elapsed} ms"))
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(
                        div()
                            .id("sql-soft-wrap")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.editor.set_soft_wrap(!this.editor.soft_wrap());
                                cx.notify();
                            }))
                            .child(if soft_wrap {
                                "Wrap: on"
                            } else {
                                "Wrap: off"
                            }),
                    )
                    .child(
                        div()
                            .id("sql-find-toggle")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.find_open = !this.find_open;
                                cx.notify();
                            }))
                            .child("Find"),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(format!(
                        "Run current: {stmt_preview}"
                    ))),
            )
            .child(if find_open {
                div()
                    .flex_none()
                    .flex()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(c.line)
                    .bg(c.raise)
                    .child(
                        div()
                            .text_xs()
                            .text_color(c.ink3)
                            .child("Find"),
                    )
                    .child(div().flex_1().child(find_input))
                    .child(
                        div()
                            .text_xs()
                            .text_color(c.ink3)
                            .child("Replace"),
                    )
                    .child(div().flex_1().child(replace_input))
                    .child(
                        div()
                            .id("sql-replace-all")
                            .cursor_pointer()
                            .text_xs()
                            .text_color(c.ink2)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let needle = this.find_input.read(cx).content().to_string();
                                let replacement = this.replace_input.read(cx).content().to_string();
                                if !needle.is_empty() {
                                    this.editor.replace_all(&needle, &replacement, true);
                                    cx.notify();
                                }
                            }))
                            .child("Replace all"),
                    )
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                div()
                    .absolute()
                    .inset_0()
                    .overflow_hidden()
                    .flex()
                    .bg(c.panel)
                    .child(
                        div()
                            .flex_none()
                            .w(gutter_w)
                            .bg(c.raise)
                            .border_r_1()
                            .border_color(c.line)
                            .children((0..line_count).map(|line| {
                                let is_current = line == primary_line;
                                let is_error = error_line == Some(line);
                                div()
                                    .h(line_h)
                                    .px_1()
                                    .text_xs()
                                    .font(mono.clone())
                                    .text_color(if is_error {
                                        c.accent
                                    } else if is_current {
                                        c.ink1
                                    } else {
                                        c.ink3
                                    })
                                    .bg(if is_error {
                                        c.line2
                                    } else if is_current {
                                        c.line2
                                    } else {
                                        c.raise
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_end()
                                    .child(format!("{}", line + 1))
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .relative()
                            .children(line_rows.into_iter().enumerate().map(|(line, (_text, segs))| {
                                let is_current = line == primary_line;
                                let _ = soft_wrap;
                                div()
                                    .absolute()
                                    .top(line_h * line as f32)
                                    .left_0()
                                    .right_0()
                                    .h(line_h)
                                    .px_2()
                                    .bg(if is_current { c.line2 } else { c.panel })
                                    .text_sm()
                                    .flex()
                                    .flex_row()
                                    .children(segs.into_iter().map(|(kind, text)| {
                                        div()
                                            .font(mono.clone())
                                            .text_color(syntax_color(kind, &syntax, ink1))
                                            .child(text)
                                    }))
                            }))
                            .child(SqlEditorElement {
                                view: cx.entity().clone(),
                            }),
                    ),
                    )
                    .child(if completion_open {
                        div()
                            .absolute()
                            .left(px(48.))
                            .bottom(px(8.))
                            .w(px(360.))
                            .max_h(px(180.))
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .bg(c.raise)
                            .border_1()
                            .border_color(c.line)
                            .rounded_md()
                            .shadow_md()
                            .children(completion_items.into_iter().enumerate().map(
                                |(ix, item)| {
                                    let selected = ix == completion_index;
                                    let detail = if let Some(table) = &item.source_table {
                                        format!("{} · {}", item.detail, table)
                                    } else {
                                        item.detail.clone()
                                    };
                                    div()
                                        .px_2()
                                        .py_1()
                                        .text_sm()
                                        .bg(if selected { c.line2 } else { c.raise })
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .font(mono.clone())
                                                .text_color(c.ink1)
                                                .child(item.label),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(c.ink3)
                                                .child(format!(
                                                    "{} · {}",
                                                    completion_kind_label(item.kind),
                                                    detail
                                                )),
                                        )
                                },
                            ))
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(if history_open {
                        div()
                            .absolute()
                            .top_0()
                            .right_0()
                            .bottom_0()
                            .w(px(300.))
                            .flex()
                            .flex_col()
                            .bg(c.raise)
                            .border_l_1()
                            .border_color(c.line)
                            .shadow_md()
                            .child(
                                div()
                                    .flex_none()
                                    .flex()
                                    .gap_1()
                                    .px_2()
                                    .py_1()
                                    .border_b_1()
                                    .border_color(c.line)
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .text_xs()
                                            .text_color(if history_tab == HistoryDrawerTab::History {
                                                c.ink1
                                            } else {
                                                c.ink3
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    this.history_tab = HistoryDrawerTab::History;
                                                    this.refresh_history_panel(cx);
                                                }),
                                            )
                                            .child("History"),
                                    )
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .text_xs()
                                            .text_color(if history_tab == HistoryDrawerTab::Snippets {
                                                c.ink1
                                            } else {
                                                c.ink3
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    this.history_tab = HistoryDrawerTab::Snippets;
                                                    this.refresh_history_panel(cx);
                                                }),
                                            )
                                            .child("Snippets"),
                                    ),
                            )
                            .child(if history_tab == HistoryDrawerTab::History {
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .flex_none()
                                            .flex()
                                            .gap_1()
                                            .p_2()
                                            .child(div().flex_1().child(history_search))
                                            .child(
                                                div()
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .text_color(c.ink2)
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(|this, _, _, cx| {
                                                            this.refresh_history_panel(cx);
                                                        }),
                                                    )
                                                    .child("Search"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .overflow_hidden()
                                            .flex()
                                            .flex_col()
                                            .children(history_entries.into_iter().map(|entry| {
                                                let sql = entry.sql.clone();
                                                div()
                                                    .px_2()
                                                    .py_1()
                                                    .border_b_1()
                                                    .border_color(c.line)
                                                    .cursor_pointer()
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(move |this, _, _, cx| {
                                                            this.insert_sql(&sql, cx);
                                                        }),
                                                    )
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font(mono.clone())
                                                            .text_color(c.ink1)
                                                            .child(entry.sql),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(c.ink3)
                                                            .child(format!(
                                                                "{} ms · {:?}",
                                                                entry.duration_ms, entry.status
                                                            )),
                                                    )
                                            })),
                                    )
                                    .into_any_element()
                            } else {
                                div()
                                    .flex_1()
                                    .overflow_hidden()
                                    .flex()
                                    .flex_col()
                                    .children(connection_snippets.into_iter().map(|snip| {
                                        let sql = snip.sql.clone();
                                        div()
                                            .px_2()
                                            .py_1()
                                            .border_b_1()
                                            .border_color(c.line)
                                            .cursor_pointer()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.insert_sql(&sql, cx);
                                                }),
                                            )
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(c.ink1)
                                                    .child(snip.name),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font(mono.clone())
                                                    .text_color(c.ink3)
                                                    .child(snip.sql),
                                            )
                                    }))
                                    .into_any_element()
                            })
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .h(px(220.))
                    .flex()
                    .flex_col()
                    .border_t_1()
                    .border_color(c.line)
                    .bg(c.raise)
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .gap_1()
                            .px_2()
                            .py_1()
                            .border_b_1()
                            .border_color(c.line)
                            .children(result_tabs.iter().enumerate().map(|(ix, tab)| {
                                let label = match tab {
                                    ResultPaneTab::Messages => SharedString::from("Messages"),
                                    ResultPaneTab::Result { label, .. } => label.clone(),
                                    ResultPaneTab::Explain { label, .. } => label.clone(),
                                };
                                let selected = ix == active_result;
                                div()
                                    .id(("sql-result-tab", ix))
                                    .cursor_pointer()
                                    .px_2()
                                    .py_0p5()
                                    .text_xs()
                                    .rounded_sm()
                                    .bg(if selected { c.line2 } else { c.raise })
                                    .text_color(if selected { c.ink1 } else { c.ink3 })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.run.active_result_tab = ix;
                                        cx.notify();
                                    }))
                                    .child(label.to_string())
                            })),
                    )
                    .child({
                        let tab = result_tabs.get(active_result);
                        match tab {
                            Some(ResultPaneTab::Messages) => div()
                                .flex_1()
                                .p_2()
                                .text_xs()
                                .font(mono.clone())
                                .text_color(c.ink2)
                                .children(run_messages.iter().map(|line| div().child(line.clone())))
                                .into_any_element(),
                            Some(ResultPaneTab::Explain { plan_text, .. }) => div()
                                .flex_1()
                                .p_2()
                                .text_xs()
                                .font(mono.clone())
                                .text_color(c.ink2)
                                .child(plan_text.clone())
                                .into_any_element(),
                            Some(ResultPaneTab::Result { .. }) => {
                                if let (Some(conn), Some(tab_ix)) =
                                    (connection_id, Some(active_result))
                                {
                                    if let Some(grid) = self.run.grid_for_tab(
                                        tab_ix,
                                        conn,
                                        bridge.clone(),
                                        self.workspace.clone(),
                                        cx,
                                    ) {
                                        div()
                                            .flex_1()
                                            .min_h_0()
                                            .child(grid)
                                            .into_any_element()
                                    } else {
                                        div()
                                            .flex_1()
                                            .child("Loading result…")
                                            .into_any_element()
                                    }
                                } else {
                                    div()
                                        .flex_1()
                                        .child("Connect to load results.")
                                        .into_any_element()
                                }
                            }
                            None => div().flex_1().into_any_element(),
                        }
                    })
                    .child(if let Some(sql) = pending_write {
                        div()
                            .absolute()
                            .inset_0()
                            .bg(gpui::rgba(0x00000080))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .w(px(420.))
                                    .p_3()
                                    .bg(c.panel)
                                    .border_1()
                                    .border_color(c.line)
                                    .rounded_md()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(c.ink1)
                                            .child("Safe mode — confirm write"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font(mono.clone())
                                            .text_color(c.ink2)
                                            .child(sql),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .justify_end()
                                            .child(
                                                div()
                                                    .id("sql-write-dismiss")
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .on_mouse_down(
                                                        gpui::MouseButton::Left,
                                                        cx.listener(|this, _, _, cx| {
                                                            this.run.pending_write_confirm = None;
                                                            cx.notify();
                                                        }),
                                                    )
                                                    .child("Cancel"),
                                            )
                                            .child(
                                                div()
                                                    .id("sql-write-confirm")
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .text_color(c.ink1)
                                                    .on_mouse_down(
                                                        gpui::MouseButton::Left,
                                                        cx.listener(|this, _, _, cx| {
                                                            this.confirm_write_run(cx);
                                                        }),
                                                    )
                                                    .child("Run write"),
                                            ),
                                    ),
                            )
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
            .child(if save_snippet_open {
                div()
                    .absolute()
                    .inset_0()
                    .bg(gpui::rgba(0x00000080))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .w(px(420.))
                            .p_3()
                            .bg(c.panel)
                            .border_1()
                            .border_color(c.line)
                            .rounded_md()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(c.ink1)
                                    .child("Save snippet"),
                            )
                            .child(div().child(snippet_name_input))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .justify_end()
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .text_xs()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    this.save_snippet_open = false;
                                                    cx.notify();
                                                }),
                                            )
                                            .child("Cancel"),
                                    )
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .text_xs()
                                            .text_color(c.ink1)
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    this.save_snippet_from_editor(cx);
                                                }),
                                            )
                                            .child("Save"),
                                    ),
                            ),
                    )
                    .into_any_element()
            } else {
                div().into_any_element()
            })
    }
}

fn completion_kind_label(kind: wisp_core::CompletionItemKind) -> &'static str {
    use wisp_core::CompletionItemKind;
    match kind {
        CompletionItemKind::Keyword => "keyword",
        CompletionItemKind::Table => "table",
        CompletionItemKind::Column => "column",
        CompletionItemKind::Function => "function",
        CompletionItemKind::Snippet => "snippet",
    }
}

fn syntax_color(
    kind: SyntaxTokenKind,
    syntax: &SyntaxColors,
    plain: gpui::Rgba,
) -> gpui::Rgba {
    match kind {
        SyntaxTokenKind::Keyword => syntax.keyword,
        SyntaxTokenKind::String => syntax.string,
        SyntaxTokenKind::Number => syntax.number,
        SyntaxTokenKind::Function => syntax.function,
        SyntaxTokenKind::Comment => syntax.comment,
        SyntaxTokenKind::Null => syntax.null,
        SyntaxTokenKind::Operator => syntax.keyword,
        SyntaxTokenKind::Plain => plain,
    }
}

pub fn bind_sql_editor_keys(cx: &mut App) {
    use gpui::KeyBinding;
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("WispSqlEditor")),
        KeyBinding::new("delete", Delete, Some("WispSqlEditor")),
        KeyBinding::new("left", Left, Some("WispSqlEditor")),
        KeyBinding::new("right", Right, Some("WispSqlEditor")),
        KeyBinding::new("home", Home, Some("WispSqlEditor")),
        KeyBinding::new("end", End, Some("WispSqlEditor")),
        KeyBinding::new("cmd-a", SelectAll, Some("WispSqlEditor")),
        KeyBinding::new("cmd-v", Paste, Some("WispSqlEditor")),
        KeyBinding::new("cmd-c", Copy, Some("WispSqlEditor")),
        KeyBinding::new("cmd-x", Cut, Some("WispSqlEditor")),
        KeyBinding::new("cmd-z", Undo, Some("WispSqlEditor")),
        KeyBinding::new("cmd-shift-z", Redo, Some("WispSqlEditor")),
        KeyBinding::new("enter", NewLine, Some("WispSqlEditor")),
        KeyBinding::new("tab", TabInsert, Some("WispSqlEditor")),
        KeyBinding::new("cmd-f", ToggleFind, Some("WispSqlEditor")),
        KeyBinding::new("cmd-g", FindNext, Some("WispSqlEditor")),
        KeyBinding::new("alt-z", ToggleSoftWrap, Some("WispSqlEditor")),
        KeyBinding::new("escape", DismissCompletion, Some("WispSqlEditor")),
        KeyBinding::new("up", CompletionUp, Some("WispSqlEditor")),
        KeyBinding::new("down", CompletionDown, Some("WispSqlEditor")),
        KeyBinding::new("cmd-enter", RunCurrent, Some("WispSqlEditor")),
        KeyBinding::new("cmd-shift-enter", RunAll, Some("WispSqlEditor")),
        KeyBinding::new("cmd-shift-h", ToggleHistoryDrawer, Some("WispSqlEditor")),
        KeyBinding::new("cmd-shift-s", SaveSnippet, Some("WispSqlEditor")),
    ]);
}
