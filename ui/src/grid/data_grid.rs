//! Virtualised read-only data grid (LUM-024).

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, uniform_list, App, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, UniformListScrollHandle, Window,
};
use wisp_core::{
    build_table_select, sql_for_display, FetchStrategy, GridStatus, PagerConfig, PageSource,
    PostgresDialect, QueryableSequentialSource, ResultPager, RowCount, SequentialPageSource,
    SortModel, TableDataQuery, Viewport,
};

use super::cell::{format_cell_text, CellKind};
use super::columns::{ColumnLayout, ROW_NUMBER_WIDTH};
use super::copy::{CopyFormat, GridCopyJob};
use super::filter_bar::GridFilterBar;
use super::layout_cache::LayoutCache;
use super::selection::{CellCoord, GridSelection};
use super::status_global::grid_status;
use crate::theme::{self, ResolvedTheme};

const ROW_HEIGHT: f32 = 26.0;
const HEADER_HEIGHT: f32 = 34.0;

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
    pager: Rc<RefCell<ResultPager<QueryableSequentialSource>>>,
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
}

impl DataGrid {
    pub fn new_table(name: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        let theme = theme::read_global(cx).resolved().clone();
        let bench = std::env::var_os("WISP_BENCH_GRID").is_some();
        let (rows, payload_cols) = if bench {
            (1_000_000u64, 19usize)
        } else {
            (50_000, 9)
        };
        let table_name: SharedString = name.into();
        let query = TableDataQuery::for_table(table_name.to_string());
        let inner = SequentialPageSource::new(rows, payload_cols);
        let columns = PageSource::columns(&inner);
        let layout = ColumnLayout::from_meta(columns);
        let source = QueryableSequentialSource::new(inner, query.clone());
        let pager = ResultPager::new(
            source,
            FetchStrategy::Offset,
            PagerConfig::default(),
        )
        .expect("pager");
        let pager = Rc::new(RefCell::new(pager));
        let filter_bar = cx.new(GridFilterBar::new);
        let sql_preview = SharedString::from(build_sql_preview(&query));
        Self {
            focus_handle: cx.focus_handle(),
            theme,
            table_name,
            query,
            filter_generation: 1,
            filter_bar,
            sql_preview,
            base_rows: rows,
            payload_cols,
            pager,
            layout,
            selection: GridSelection::None,
            scroll: UniformListScrollHandle::default(),
            scroll_x: 0.0,
            resize: None,
            layout_cache: LayoutCache::default(),
            copy_job: GridCopyJob::default(),
            query_ms: if bench { 42 } else { 8 },
            last_viewport: (0, 40),
            pages_loaded: true,
        }
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

    fn cell_label(&mut self, row: u64, col: usize) -> (CellKind, SharedString) {
        if let Some(hit) = self.layout_cache.get(row, col as u16) {
            return (CellKind::Text, hit);
        }
        let type_name = self.layout.columns[col].meta.type_name.clone();
        let (text, kind) = {
            let pager = self.pager.borrow();
            let text = pager.cell_text(row, col).ok().flatten();
            let value = pager.cell_value(row, col).ok().flatten();
            let display = format_cell_text(text.as_deref(), &type_name, value.as_ref());
            (display.text, display.kind)
        };
        let shared: SharedString = text.into();
        self.layout_cache.put(row, col as u16, shared.clone());
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
            row_to: last.min(self.total_rows().saturating_sub(1)),
            total_rows: total,
            total_estimate: estimate,
            query_ms: self.query_ms,
        });
    }

    fn apply_filters(&mut self, cx: &mut Context<Self>) {
        let mut next = self.filter_bar.read(cx).build_query(&self.table_name, cx);
        next.sort = self.query.sort.clone();
        self.query = next;
        self.filter_generation = self.filter_generation.wrapping_add(1);
        let inner = SequentialPageSource::new(self.base_rows, self.payload_cols);
        let source = QueryableSequentialSource::new(inner, self.query.clone());
        if let Ok(pager) = ResultPager::new(
            source,
            FetchStrategy::Offset,
            PagerConfig::default(),
        ) {
            *self.pager.borrow_mut() = pager;
        }
        self.layout_cache = LayoutCache::default();
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
        let total = self.total_rows() as usize;
        let visible_cols = self.layout.visible_indices();
        let scroll_x = self.scroll_x;
        let selection = self.selection.clone();
        let scroll = self.scroll.clone();
        let layout = self.layout.clone();

        let sort = self.query.sort.clone();
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

        div()
            .id("data-grid")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(c.panel.clone())
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.platform {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "c" => this.copy_selection(cx, CopyFormat::Tsv),
                    _ => {}
                }
            }))
            .child(filter_bar)
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
                uniform_list(
                    "grid-rows",
                    total,
                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                        let visible = range.len().max(1) as u32;
                        let first = range.start as u64;
                        this.sync_viewport(first, visible);
                        let mut rows = Vec::new();
                        for row_ix in range {
                            let row = row_ix as u64;
                            rows.push(render_row(
                                cx,
                                &theme,
                                row,
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
            )
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
    row: u64,
    layout: &ColumnLayout,
    visible_cols: &[usize],
    scroll_x: f32,
    selection: &GridSelection,
    grid: &mut DataGrid,
) -> impl IntoElement {
    let c = &theme.colors;
    let selected_row = matches!(selection, GridSelection::Row(r) if *r == row)
        || matches!(selection, GridSelection::Range { start, end } if row >= start.row && row <= end.row);
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
        .id(("grid-row", row as u32))
        .h(px(ROW_HEIGHT))
        .flex()
        .w_full()
        .bg(if selected_row {
            c.accent_soft.clone()
        } else {
            c.panel.clone()
        })
        .child(
            div()
                .id(("grid-rn", row as u32))
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
                    this.selection = GridSelection::Row(row);
                    cx.notify();
                }))
                .child(format!("{}", row + 1)),
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
                            let selected = selection.contains(row, col_ix);
                            let (kind, label) = grid.cell_label(row, col_ix);
                            div()
                                            .id(("grid-cell", row as u32 * 1000 + col_ix as u32))
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
                                    c.panel.clone()
                                })
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                                    let coord = CellCoord { row, col: col_ix };
                                    this.selection = if event.modifiers().shift {
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
                                    cx.notify();
                                }))
                                .child(label)
                        })),
                ),
        )
}
