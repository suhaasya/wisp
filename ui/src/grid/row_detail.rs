//! Right-hand row detail panel (LUM-027).

use std::collections::{HashMap, HashSet};

use gpui::{
    div, prelude::*, px, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled,
};
use wisp_core::{
    mock_cell_needs_lazy_fetch, mock_cell_preview, mock_layout_for_table, pretty_format_value,
    relations_for_table, reverse_references, ForeignKey, RowDetailField,
};

use super::selection::GridSelection;
use crate::components::{select, toggle};
use crate::theme::ResolvedTheme;
use super::edit::editor_kind_for_column;

pub const DEFAULT_DETAIL_WIDTH: f32 = 300.0;
pub const MIN_DETAIL_WIDTH: f32 = 220.0;
pub const MAX_DETAIL_WIDTH: f32 = 520.0;

#[derive(Debug, Clone)]
pub struct RowDetailState {
    pub open: bool,
    pub width: f32,
    pub expanded_fields: HashSet<usize>,
    pub loaded_full: HashMap<(u64, usize), String>,
    pub panel_resize: Option<f32>,
    pub enum_select_open: Option<usize>,
}

impl Default for RowDetailState {
    fn default() -> Self {
        Self {
            open: true,
            width: DEFAULT_DETAIL_WIDTH,
            expanded_fields: HashSet::new(),
            loaded_full: HashMap::new(),
            panel_resize: None,
            enum_select_open: None,
        }
    }
}

impl RowDetailState {
    pub fn new_default() -> Self {
        Self::default()
    }
    pub fn clear_large_values(&mut self) {
        self.loaded_full.clear();
        self.expanded_fields.clear();
    }

    pub fn selected_row(selection: &GridSelection) -> Option<u64> {
        match selection {
            GridSelection::None => None,
            GridSelection::Cell(c) => Some(c.row),
            GridSelection::Row(r) => Some(*r),
            GridSelection::Range { start, .. } => Some(start.row),
        }
    }
}

pub fn row_fields_from_pager(
    previews: impl IntoIterator<Item = (String, String, String, bool)>,
) -> Vec<RowDetailField> {
    previews
        .into_iter()
        .map(|(name, ty, preview, is_pk)| {
            let is_null = preview == "NULL";
            let is_truncated = preview.ends_with('…') || preview.len() >= wisp_core::LARGE_VALUE_THRESHOLD;
            RowDetailField {
                name,
                type_name: ty,
                preview,
                is_null,
                is_truncated,
                is_pk,
            }
        })
        .collect()
}

pub fn row_fields_from_cache(
    table: &str,
    row: u64,
    column_names: &[(String, String)],
) -> Vec<RowDetailField> {
    let mock = mock_layout_for_table(table);
    if mock.table != table {
        return column_names
            .iter()
            .map(|(name, ty)| RowDetailField {
                name: name.clone(),
                type_name: ty.clone(),
                preview: "—".into(),
                is_null: false,
                is_truncated: false,
                is_pk: false,
            })
            .collect();
    }
    mock.columns
        .iter()
        .map(|c| {
            let preview = mock_cell_preview(table, row, c);
            let is_null = preview == "NULL";
            let is_truncated = mock_cell_needs_lazy_fetch(c, &preview);
            RowDetailField {
                name: c.meta.name.clone(),
                type_name: c.meta.type_name.clone(),
                preview,
                is_null,
                is_truncated,
                is_pk: c.meta.is_pk,
            }
        })
        .collect()
}

pub fn load_full_value(table: &str, row: u64, col_ix: usize) -> Option<String> {
    let mock = mock_layout_for_table(table);
    let col = mock.columns.get(col_ix)?;
    let full = match wisp_core::mock_cell_display(table, row, col) {
        None => return Some("NULL".into()),
        Some(s) => s,
    };
    Some(pretty_format_value(&col.meta.type_name, &full))
}

#[derive(Clone)]
pub struct FkNavigate {
    pub target_table: SharedString,
    pub fk: ForeignKey,
    pub key_values: Vec<String>,
}

pub fn fk_actions(table: &str, row: u64, fields: &[RowDetailField]) -> Vec<(String, FkNavigate)> {
    let Some(schema) = relations_for_table(table) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (col_name, fk) in schema.foreign_keys {
        let mut values = Vec::new();
        let mut ok = true;
        for part in &fk.columns {
            let Some(field) = fields.iter().find(|f| f.name == part.local_column) else {
                ok = false;
                break;
            };
            if field.is_null {
                ok = false;
                break;
            }
            values.push(field.preview.clone());
        }
        if ok {
            out.push((
                col_name,
                FkNavigate {
                    target_table: fk.target_table.clone().into(),
                    fk,
                    key_values: values,
                },
            ));
        }
    }
    let _ = row;
    out
}

pub fn render_row_detail_panel(
    cx: &mut Context<super::data_grid::DataGrid>,
    theme: &ResolvedTheme,
    table: &str,
    row: Option<u64>,
    fields: &[RowDetailField],
    state: &RowDetailState,
    editing_enabled: bool,
    on_fk: impl Fn(&mut super::data_grid::DataGrid, FkNavigate, &mut Context<super::data_grid::DataGrid>)
        + Clone
        + 'static,
) -> impl IntoElement {
    let c = &theme.colors;
    let reverse = reverse_references(table);
    let width = state.width;

    div()
        .id("row-detail-panel")
        .flex_none()
        .w(px(width))
        .h_full()
        .flex()
        .flex_col()
        .border_l_1()
        .border_color(c.line.clone())
        .bg(c.raise.clone())
        .child(
            div()
                .flex_none()
                .px_2()
                .py_1()
                .border_b_1()
                .border_color(c.line2.clone())
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(match row {
                    Some(r) => format!("Row {}", r + 1),
                    None => "Row detail".into(),
                }),
        )
        .child(if row.is_none() {
            div()
                .flex_1()
                .p_2()
                .text_sm()
                .text_color(c.ink3.clone())
                .child("Select a row to inspect values.")
                .into_any_element()
        } else {
            let row = row.expect("checked");
            div()
                .id("row-detail-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_2()
                .p_2()
                .children(fields.iter().enumerate().map(|(ix, field)| {
                    let table_for_load = table.to_string();
                    let expanded = state.expanded_fields.contains(&ix);
                    let display = state
                        .loaded_full
                        .get(&(row, ix))
                        .cloned()
                        .unwrap_or_else(|| field.preview.clone());
                    let show_multiline = expanded
                        || field.type_name.contains("json")
                        || field.is_truncated;
                    div()
                        .id(("rd-field", ix as u32))
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_xs()
                                .text_color(c.ink3.clone())
                                .child(format!(
                                    "{} · {}",
                                    field.name, field.type_name
                                )),
                        )
                        .child(if show_multiline {
                            div()
                                .text_xs()
                                .font(theme.typography.gpui_mono_font())
                                .text_color(c.ink1.clone())
                                .whitespace_normal()
                                .child(display)
                                .into_any_element()
                        } else {
                            div()
                                .text_sm()
                                .font(theme.typography.gpui_mono_font())
                                .text_color(c.ink1.clone())
                                .child(display)
                                .into_any_element()
                        })
                        .when(field.is_truncated && !state.loaded_full.contains_key(&(row, ix)), |el| {
                            el.child(
                                div()
                                    .id(("rd-load", ix as u32))
                                    .text_xs()
                                    .text_color(c.accent.clone())
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(text) =
                                            load_full_value(&table_for_load, row, ix)
                                        {
                                            this.detail.loaded_full.insert((row, ix), text);
                                            cx.notify();
                                        }
                                    }))
                                    .child("Load full value"),
                            )
                        })
                        .when(editing_enabled && !field.is_pk, |el| {
                            let kind = editor_kind_for_column(&field.type_name, &field.name);
                            el.when(matches!(kind, super::edit::EditorKind::Boolean), |el| {
                                let on = field.preview == "true" || field.preview == "1";
                                el.child(toggle::toggle(
                                    cx,
                                    "rd-bool",
                                    "Value",
                                    on,
                                    theme,
                                    move |this, _, cx| {
                                        this.stage_display_cell(
                                            row,
                                            ix,
                                            if on { "false" } else { "true" },
                                            cx,
                                        );
                                    },
                                ))
                            })
                            .when(matches!(kind, super::edit::EditorKind::Enum(_)), |el| {
                                let options = ["pending", "paid", "shipped", "cancelled"];
                                let selected = options
                                    .iter()
                                    .position(|o| *o == field.preview.as_str())
                                    .unwrap_or(0);
                                let open = state.enum_select_open == Some(ix);
                                el.child(select::select(
                                    cx,
                                    "rd-enum",
                                    "Status",
                                    &options,
                                    selected,
                                    open,
                                    theme,
                                    move |this, _, cx| {
                                        this.detail.enum_select_open =
                                            if open { None } else { Some(ix) };
                                        cx.notify();
                                    },
                                    move |this, pick, _, cx| {
                                        if let Some(v) = options.get(pick) {
                                            this.stage_display_cell(row, ix, v, cx);
                                        }
                                        this.detail.enum_select_open = None;
                                        cx.notify();
                                    },
                                ))
                            })
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .text_xs()
                                    .text_color(c.accent.clone())
                                    .child(
                                        div()
                                            .id(("rd-null", ix as u32))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.set_cell_null(row, ix, cx);
                                            }))
                                            .child("NULL"),
                                    )
                                    .child(
                                        div()
                                            .id(("rd-default", ix as u32))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.set_cell_default(row, ix, cx);
                                            }))
                                            .child("DEFAULT"),
                                    )
                                    .child(
                                        div()
                                            .id(("rd-revert", ix as u32))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.revert_display_cell(row, ix, cx);
                                            }))
                                            .child("Revert"),
                                    ),
                            )
                        })
                        .into_any_element()
                }))
                .children(fk_actions(table, row, fields).into_iter().enumerate().map(|(fk_ix, (col, nav))| {
                    let on_fk = on_fk.clone();
                    let target = nav.target_table.clone();
                    div()
                        .id(("rd-fk", fk_ix as u32))
                        .text_xs()
                        .text_color(c.accent.clone())
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            on_fk(this, nav.clone(), cx);
                        }))
                        .child(format!("Open related row → {target} ({col})"))
                        .into_any_element()
                }))
                .when(!reverse.is_empty(), |el| {
                    el.child(
                        div()
                            .mt_2()
                            .pt_2()
                            .border_t_1()
                            .border_color(c.line2.clone())
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(c.ink2.clone())
                                    .child("Rows that point here"),
                            )
                            .children(reverse.iter().map(|rev| {
                                div()
                                    .text_xs()
                                    .text_color(c.ink2.clone())
                                    .child(format!(
                                        "{} · {} rows",
                                        rev.from_table, rev.row_count
                                    ))
                                    .into_any_element()
                            })),
                    )
                })
                .into_any_element()
        })
}

pub fn detail_resize_handle(
    cx: &mut Context<super::data_grid::DataGrid>,
) -> impl IntoElement {
    div()
        .id("row-detail-resize")
        .w(px(4.0))
        .h_full()
        .flex_none()
        .cursor_col_resize()
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(|this, event: &gpui::MouseDownEvent, _, _| {
                this.detail.panel_resize =
                    Some(f32::from(event.position.x) + this.detail.width);
            }),
        )
}
