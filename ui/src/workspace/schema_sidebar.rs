//! Virtualised schema object tree (LUM-022).

use std::rc::Rc;

use std::collections::HashSet;

use gpui::{
    div, prelude::*, px, uniform_list, App, ClipboardItem, Context, Entity, FocusHandle,
    Focusable, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, UniformListScrollHandle, WeakEntity, Window,
};
use wisp_core::{
    build_sidebar_rows, catalog_disconnected, fuzzy_match_highlight_indices, SchemaCatalog,
    SchemaObjectKind, SchemaSidebarPrefs, SchemaSidebarStateStore, SidebarRow,
};
use wisp_core::ConnectionId;

use crate::components::text_input::{TextInput, TextInputKind};
use crate::theme::{self, ResolvedTheme};

const ROW_H: f32 = 28.0;
const GROUP_ROW_H: f32 = 30.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaLoadState {
    Idle,
    Loading,
    Ready,
    Failed(SharedString),
}

pub struct SchemaSidebar {
    focus_handle: FocusHandle,
    theme: ResolvedTheme,
    workspace: WeakEntity<super::view::WorkspaceView>,
    connection_id: Option<ConnectionId>,
    catalog: Rc<SchemaCatalog>,
    prefs_store: SchemaSidebarStateStore,
    filter_input: Entity<TextInput>,
    filter_text: String,
    scroll: UniformListScrollHandle,
    rows: Rc<Vec<SidebarRow>>,
    selected_row: usize,
    /// Stable key for expand/collapse prefs before a session is bound.
    demo_connection_id: ConnectionId,
    load_state: SchemaLoadState,
}

impl SchemaSidebar {
    pub fn new(cx: &mut Context<Self>, workspace: WeakEntity<super::view::WorkspaceView>) -> Self {
        let theme = theme::read_global(cx).resolved().clone();
        let catalog = Rc::new(catalog_disconnected());
        let mut prefs = SchemaSidebarPrefs::default();
        prefs.schema = "public".into();
        prefs.expanded.insert(SchemaObjectKind::Tables);
        prefs.expanded.insert(SchemaObjectKind::Views);
        let filter_input = cx.new(|cx| {
            TextInput::new(
                cx,
                "Filter objects",
                TextInputKind::SingleLine,
                theme.clone(),
            )
        });
        let rows = Rc::new(build_sidebar_rows(
            &catalog,
            &prefs.schema,
            &prefs,
            "",
        ));
        Self {
            focus_handle: cx.focus_handle(),
            theme,
            workspace,
            connection_id: None,
            catalog,
            prefs_store: SchemaSidebarStateStore::default(),
            filter_input,
            filter_text: String::new(),
            scroll: UniformListScrollHandle::default(),
            rows,
            selected_row: 0,
            demo_connection_id: ConnectionId::new_v7(),
            load_state: SchemaLoadState::Idle,
        }
    }

    fn store_id(&self) -> ConnectionId {
        self.connection_id.unwrap_or(self.demo_connection_id)
    }

    pub fn catalog(&self) -> Rc<SchemaCatalog> {
        Rc::clone(&self.catalog)
    }

    pub fn active_schema(&self) -> String {
        let id = self.store_id();
        self.prefs_store
            .state(id)
            .prefs
            .schema
            .clone()
    }

    pub fn set_connection(&mut self, id: ConnectionId, label: &SharedString, cx: &mut Context<Self>) {
        self.connection_id = Some(id);
        let state = self.prefs_store.state(id);
        self.selected_row = state.selected_row;
        let mut catalog = (*self.catalog).clone();
        catalog.connection_label = label.to_string();
        self.catalog = Rc::new(catalog);
        self.rebuild_rows(id, cx);
    }

    /// Replace demo/fixture catalog with live introspection (LUM-021).
    pub fn set_load_state(&mut self, state: SchemaLoadState, cx: &mut Context<Self>) {
        self.load_state = state;
        self.rebuild_rows(self.store_id(), cx);
    }

    pub fn set_catalog(&mut self, catalog: Rc<SchemaCatalog>, cx: &mut Context<Self>) {
        self.catalog = catalog;
        let id = self.store_id();
        let prefs = &mut self.prefs_store.state_mut(id).prefs;
        if !self.catalog.schemas.iter().any(|s| s == &prefs.schema) {
            prefs.schema = self
                .catalog
                .schemas
                .first()
                .cloned()
                .unwrap_or_else(|| "public".into());
        }
        self.rebuild_rows(id, cx);
    }

    fn rebuild_rows(&mut self, id: ConnectionId, cx: &mut Context<Self>) {
        let prefs = self.prefs_store.state(id).prefs.clone();
        self.rows = Rc::new(build_sidebar_rows(
            &self.catalog,
            &prefs.schema,
            &prefs,
            &self.filter_text,
        ));
        self.selected_row = self
            .selected_row
            .min(self.rows.len().saturating_sub(1));
        cx.notify();
    }

    fn sync_filter(&mut self, cx: &mut Context<Self>) {
        let next = self.filter_input.read(cx).content().to_string();
        if next == self.filter_text {
            return;
        }
        self.filter_text = next;
        self.rebuild_rows(self.store_id(), cx);
    }

    fn toggle_group(&mut self, kind: SchemaObjectKind, cx: &mut Context<Self>) {
        let id = self.store_id();
        self.prefs_store
            .state_mut(id)
            .prefs
            .toggle_group(kind);
        self.rebuild_rows(id, cx);
    }

    fn cycle_schema(&mut self, cx: &mut Context<Self>) {
        let id = self.store_id();
        let prefs = &mut self.prefs_store.state_mut(id).prefs;
        if self.catalog.schemas.is_empty() {
            return;
        }
        let ix = self
            .catalog
            .schemas
            .iter()
            .position(|x| x == &prefs.schema)
            .unwrap_or(0);
        let next = (ix + 1) % self.catalog.schemas.len();
        prefs.schema = self.catalog.schemas[next].clone();
        self.rebuild_rows(id, cx);
    }

    fn activate_row(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        self.prefs_store
            .state_mut(self.store_id())
            .selected_row = index;
        self.selected_row = index;
        if let SidebarRow::Object { name, .. } = row {
            let workspace = self.workspace.clone();
            let name = SharedString::from(name.clone());
            cx.defer(move |cx| {
                workspace
                    .update(cx, |ws, cx| ws.open_table_data(name, cx))
                    .ok();
            });
        }
        cx.notify();
    }

    fn copy_selected_name(&mut self, cx: &mut Context<Self>) {
        let Some(SidebarRow::Object { name, .. }) = self.rows.get(self.selected_row) else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(name.clone()));
    }
}

impl Focusable for SchemaSidebar {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SchemaSidebar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_filter(cx);
        let theme = self.theme.clone();
        let c = theme.colors.clone();
        let prefs = self
            .connection_id
            .map(|id| self.prefs_store.state(id).prefs.clone())
            .unwrap_or_else(SchemaSidebarPrefs::default);
        let db_label = if self.catalog.database.is_empty() {
            self.catalog.connection_label.clone()
        } else {
            self.catalog.database.clone()
        };
        let schema_label = if prefs.schema.is_empty() {
            "—".into()
        } else {
            prefs.schema.clone()
        };
        let rows = Rc::clone(&self.rows);
        let scroll = self.scroll.clone();
        let selected = self.selected_row;
        let filter_query = self.filter_text.clone();
        let empty_hint = match &self.load_state {
            SchemaLoadState::Idle if self.connection_id.is_none() => {
                "Select a connection to browse schema.".into()
            }
            SchemaLoadState::Loading => "Loading schema from database…".into(),
            SchemaLoadState::Failed(msg) => format!("Schema load failed: {msg}"),
            SchemaLoadState::Ready if rows.is_empty() => "No objects in this schema.".into(),
            SchemaLoadState::Idle if self.connection_id.is_some() && rows.is_empty() => {
                "Waiting for schema…".into()
            }
            _ => String::new(),
        };
        let show_empty = rows.is_empty() && !empty_hint.is_empty();

        div()
            .id("schema-sidebar")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(c.sidebar.clone())
            .border_r_1()
            .border_color(c.line.clone())
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_2()
                    .border_b_1()
                    .border_color(c.line2)
                    .child(
                        div()
                            .id("schema-selector")
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(c.line.clone())
                            .bg(c.panel.clone())
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.cycle_schema(cx)))
                            .child(
                                div()
                                    .text_sm()
                                    .child(
                                        div()
                                            .flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .child(db_label),
                                            )
                                            .child(
                                                div()
                                                    .text_color(c.ink3.clone())
                                                    .child(format!("/ {schema_label}")),
                                            ),
                                    ),
                            )
                            .child(div().text_color(c.ink3).text_xs().child("▾")),
                    )
                    .child(self.filter_input.clone()),
            )
            .children(if show_empty {
                vec![div()
                    .flex_1()
                    .p_3()
                    .text_sm()
                    .text_color(c.ink3.clone())
                    .child(empty_hint)]
            } else {
                vec![]
            })
            .child(
                uniform_list(
                    "schema-tree",
                    rows.len(),
                    cx.processor(move |_this, range, _window, cx| {
                        let mut elements = Vec::new();
                        for index in range {
                            let Some(row) = rows.get(index) else {
                                continue;
                            };
                            let active = index == selected;
                            match row {
                                SidebarRow::Group {
                                    kind,
                                    count,
                                    expanded,
                                } => {
                                    let chevron = if *expanded { "▾" } else { "▸" };
                                    let label = kind.label();
                                    let kind = *kind;
                                    elements.push(
                                        div()
                                            .id(("schema-grp", index as u32))
                                            .h(px(GROUP_ROW_H))
                                            .flex()
                                            .items_center()
                                            .px_2()
                                            .gap_2()
                                            .cursor_pointer()
                                            .bg(if active {
                                                c.accent_soft
                                            } else {
                                                c.sidebar
                                            })
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(c.ink2)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.selected_row = index;
                                                this.toggle_group(kind, cx);
                                            }))
                                            .child(format!("{chevron} {label}"))
                                            .child(
                                                div()
                                                    .ml_auto()
                                                    .text_xs()
                                                    .text_color(c.ink3)
                                                    .child(format!("{count}")),
                                            )
                                            .into_any_element(),
                                    );
                                }
                                SidebarRow::Object {
                                    name,
                                    object_kind,
                                    ..
                                } => {
                                    let name_str = name.clone();
                                    let name = SharedString::from(name_str.clone());
                                    let is_view = *object_kind == SchemaObjectKind::Views;
                                    let filter_query = filter_query.clone();
                                    elements.push(
                                        div()
                                            .id(("schema-obj", index as u32))
                                            .h(px(ROW_H))
                                            .flex()
                                            .items_center()
                                            .pl_4()
                                            .pr_2()
                                            .gap_2()
                                            .cursor_pointer()
                                            .bg(if active {
                                                c.accent_soft
                                            } else {
                                                c.sidebar
                                            })
                                            .text_sm()
                                            .text_color(c.ink1)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.activate_row(index, cx);
                                            }))
                                            .on_mouse_down(
                                                gpui::MouseButton::Right,
                                                cx.listener({
                                                    let name = name.clone();
                                                    move |this, _, _, cx| {
                                                        this.workspace
                                                            .update(cx, |ws, cx| {
                                                                ws.open_structure(name.clone(), cx);
                                                            })
                                                            .ok();
                                                    }
                                                }),
                                            )
                                            .child(
                                                div()
                                                    .size(px(8.0))
                                                    .rounded_xs()
                                                    .bg(if is_view {
                                                        c.accent
                                                    } else {
                                                        c.line
                                                    }),
                                            )
                                            .child(highlighted_name(
                                                &name_str,
                                                &filter_query,
                                                c.accent.clone(),
                                                c.ink1.clone(),
                                            ))
                                            .into_any_element(),
                                    );
                                }
                            }
                        }
                        elements
                    }),
                )
                .flex_1()
                .track_scroll(&scroll)
                .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                    let key = event.keystroke.key.as_str();
                    let len = this.rows.len();
                    if len == 0 {
                        return;
                    }
                    match key {
                        "up" | "k" if this.selected_row > 0 => {
                            this.selected_row -= 1;
                        }
                        "down" | "j" if this.selected_row + 1 < len => {
                            this.selected_row += 1;
                        }
                        "enter" | "return" => {
                            let row = this.selected_row;
                            this.activate_row(row, cx);
                            return;
                        }
                        "c" if event.keystroke.modifiers.platform && !event.keystroke.modifiers.shift
                        =>
                        {
                            this.copy_selected_name(cx);
                            return;
                        }
                        _ => return,
                    }
                    cx.notify();
                })),
            )
    }
}

fn highlighted_name(
    name: &str,
    query: &str,
    accent: gpui::Rgba,
    base: gpui::Rgba,
) -> impl IntoElement {
    let highlight: HashSet<usize> =
        fuzzy_match_highlight_indices(name, query).into_iter().collect();
    if highlight.is_empty() {
        return div().text_color(base).child(name.to_string()).into_any_element();
    }
    let mut segments: Vec<(gpui::Rgba, String)> = Vec::new();
    for (i, ch) in name.chars().enumerate() {
        let color = if highlight.contains(&i) {
            accent
        } else {
            base
        };
        if segments.last().map(|(c, _)| *c == color).unwrap_or(false) {
            segments.last_mut().unwrap().1.push(ch);
        } else {
            segments.push((color, ch.to_string()));
        }
    }
    div().flex().children(segments.into_iter().map(|(color, text)| {
        div().text_color(color).child(text)
    }))
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use wisp_core::{build_sidebar_rows, demo_catalog_large_tables, SchemaObjectKind, SchemaSidebarPrefs};

    #[test]
    fn ten_k_mock_sidebar_row_count() {
        let catalog = demo_catalog_large_tables(10_000);
        let mut prefs = SchemaSidebarPrefs::default();
        prefs.schema = "public".into();
        prefs.expanded.insert(SchemaObjectKind::Tables);
        let rows = build_sidebar_rows(&catalog, "public", &prefs, "");
        assert_eq!(rows.len(), 10_001);
    }
}
