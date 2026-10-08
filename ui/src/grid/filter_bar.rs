//! Filter row UI (column, operator, value, AND/OR, raw WHERE).

use gpui::{
    div, prelude::*, px, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window,
};
use wisp_core::{FilterCombine, FilterOperator, FilterTerm, TableDataQuery};

use crate::components::text_input::{TextInput, TextInputKind};
use crate::theme::{self, ResolvedTheme};

struct FilterRowUi {
    column_input: Entity<TextInput>,
    value_input: Entity<TextInput>,
    value_to_input: Entity<TextInput>,
    operator_index: usize,
}

pub struct GridFilterBar {
    theme: ResolvedTheme,
    rows: Vec<FilterRowUi>,
    raw_input: Entity<TextInput>,
    combine_and: bool,
    raw_mode: bool,
}

impl GridFilterBar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let theme = theme::read_global(cx).resolved().clone();
        Self {
            theme: theme.clone(),
            rows: vec![Self::new_row(cx, &theme)],
            raw_input: cx.new(|cx| {
                TextInput::new(cx, "Raw WHERE…", TextInputKind::SingleLine, theme.clone())
            }),
            combine_and: true,
            raw_mode: false,
        }
    }

    fn new_row(cx: &mut Context<Self>, theme: &ResolvedTheme) -> FilterRowUi {
        FilterRowUi {
            column_input: cx.new(|cx| {
                TextInput::new(cx, "Column", TextInputKind::SingleLine, theme.clone())
            }),
            value_input: cx.new(|cx| {
                TextInput::new(cx, "Value", TextInputKind::SingleLine, theme.clone())
            }),
            value_to_input: cx.new(|cx| {
                TextInput::new(cx, "To", TextInputKind::SingleLine, theme.clone())
            }),
            operator_index: 0,
        }
    }

    pub fn load_query(&mut self, query: &TableDataQuery, cx: &mut Context<Self>) {
        self.combine_and = query.filter.combine == FilterCombine::And;
        self.raw_mode = query
            .filter
            .raw_where
            .as_ref()
            .is_some_and(|s| !s.is_empty());
        if let Some(raw) = &query.filter.raw_where {
            self.raw_input.update(cx, |i, cx| {
                i.set_content(raw);
                cx.notify();
            });
        }
        let terms = if query.filter.terms.is_empty() {
            vec![FilterTerm::default()]
        } else {
            query.filter.terms.clone()
        };
        while self.rows.len() < terms.len() {
            let theme = self.theme.clone();
            self.rows.push(Self::new_row(cx, &theme));
        }
        while self.rows.len() > terms.len() {
            self.rows.pop();
        }
        for (row, term) in self.rows.iter_mut().zip(terms.iter()) {
            row.column_input.update(cx, |i, cx| {
                i.set_content(&term.column);
                cx.notify();
            });
            row.operator_index = FilterOperator::UI_OPTIONS
                .iter()
                .position(|(_, op)| *op == term.operator)
                .unwrap_or(0);
            row.value_input.update(cx, |i, cx| {
                i.set_content(&term.value);
                cx.notify();
            });
            row.value_to_input.update(cx, |i, cx| {
                i.set_content(&term.value_to);
                cx.notify();
            });
        }
    }

    fn add_row(&mut self, cx: &mut Context<Self>) {
        let theme = self.theme.clone();
        self.rows.push(Self::new_row(cx, &theme));
        cx.notify();
    }

    fn remove_row(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.rows.len() <= 1 {
            return;
        }
        self.rows.remove(ix);
        cx.notify();
    }

    pub fn build_query(&self, table: &str, cx: &gpui::App) -> TableDataQuery {
        let mut query = TableDataQuery::for_table(table);
        query.filter.combine = if self.combine_and {
            FilterCombine::And
        } else {
            FilterCombine::Or
        };
        if self.raw_mode {
            let raw = self.raw_input.read(cx).content().to_string();
            query.filter.raw_where = Some(raw);
            return query;
        }
        for row in &self.rows {
            let op = FilterOperator::UI_OPTIONS[row.operator_index].1;
            let column = row.column_input.read(cx).content().to_string();
            let value = row.value_input.read(cx).content().to_string();
            let value_to = row.value_to_input.read(cx).content().to_string();
            if column.is_empty() {
                continue;
            }
            if op.needs_value() && value.is_empty() {
                continue;
            }
            query.filter.terms.push(FilterTerm {
                column,
                operator: op,
                value,
                value_to,
            });
        }
        query
    }
}

impl Render for GridFilterBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = &self.theme.colors;
        let row_count = self.rows.len();

        div()
            .id("grid-filter-bar")
            .flex_none()
            .flex()
            .flex_col()
            .gap_1()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(c.line.clone())
            .bg(c.raise.clone())
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("filter-combine")
                            .flex()
                            .rounded_md()
                            .border_1()
                            .border_color(c.line.clone())
                            .child(combine_btn(cx, c, "AND", self.combine_and, FilterCombine::And))
                            .child(combine_btn(cx, c, "OR", !self.combine_and, FilterCombine::Or)),
                    )
                    .child(
                        div()
                            .id("filter-raw-toggle")
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .bg(if self.raw_mode {
                                c.accent_soft.clone()
                            } else {
                                c.panel.clone()
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.raw_mode = !this.raw_mode;
                                cx.notify();
                            }))
                            .child(if self.raw_mode {
                                "Raw WHERE"
                            } else {
                                "Builder"
                            }),
                    )
                    .when(!self.raw_mode, |el| {
                        el.child(
                            div()
                                .id("filter-add-row")
                                .px_2()
                                .py_0p5()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .border_1()
                                .border_color(c.line.clone())
                                .on_click(cx.listener(|this, _, _, cx| this.add_row(cx)))
                                .child("+ Filter"),
                        )
                    }),
            )
            .when(self.raw_mode, |el| el.child(self.raw_input.clone()))
            .when(!self.raw_mode, |el| {
                el.children((0..row_count).map(|ix| {
                    let row = &self.rows[ix];
                    let op_label = FilterOperator::UI_OPTIONS[row.operator_index].0;
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(120.0))
                                .child(row.column_input.clone()),
                        )
                        .child(
                            div()
                                .id(("filter-op", ix as u32))
                                .px_2()
                                .py_0p5()
                                .rounded_md()
                                .border_1()
                                .border_color(c.line.clone())
                                .text_xs()
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.rows[ix].operator_index =
                                        (this.rows[ix].operator_index + 1)
                                            % FilterOperator::UI_OPTIONS.len();
                                    cx.notify();
                                }))
                                .child(op_label),
                        )
                        .child(
                            div()
                                .w(px(160.0))
                                .child(row.value_input.clone()),
                        )
                        .child(
                            div()
                                .w(px(120.0))
                                .child(row.value_to_input.clone()),
                        )
                        .when(row_count > 1, |row_el| {
                            row_el.child(
                                div()
                                    .id(("filter-rm", ix as u32))
                                    .px_1()
                                    .text_xs()
                                    .cursor_pointer()
                                    .text_color(c.ink3.clone())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.remove_row(ix, cx);
                                    }))
                                    .child("×"),
                            )
                        })
                }))
            })
    }
}

fn combine_btn(
    cx: &mut Context<GridFilterBar>,
    c: &theme::ResolvedColors,
    label: &'static str,
    active: bool,
    mode: FilterCombine,
) -> impl IntoElement {
    div()
        .id(label)
        .px_2()
        .py_0p5()
        .text_xs()
        .cursor_pointer()
        .bg(if active {
            c.panel.clone()
        } else {
            c.raise.clone()
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            this.combine_and = mode == FilterCombine::And;
            cx.notify();
        }))
        .child(label)
}
