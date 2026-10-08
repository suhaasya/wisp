//! Workspace layout (sidebar + tabs + main pane).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window,
};
use wisp_core::{
    ConnectionHub, ConnectionId, SavedWorkspaceTab, SavedWorkspaceTabKind, WorkspaceSessionStore,
    WorkspaceTabState,
};

use super::schema_sidebar::SchemaSidebar;
use crate::grid::DataGrid;
use crate::theme::{self, ResolvedTheme};

const SIDEBAR_WIDTH: f32 = 240.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTab {
    pub id: u32,
    pub title: SharedString,
    pub kind: WorkspaceTabKind,
    pub pinned: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTabKind {
    TableData,
    TableStructure,
    Query,
}

pub struct WorkspaceView {
    focus_handle: FocusHandle,
    hub: Arc<ConnectionHub>,
    sessions: Arc<Mutex<WorkspaceSessionStore>>,
    connection_id: Option<ConnectionId>,
    connection_label: SharedString,
    schema_sidebar: Entity<SchemaSidebar>,
    tabs: Vec<WorkspaceTab>,
    active_tab: usize,
    next_tab_id: u32,
    table_grids: HashMap<u32, Entity<DataGrid>>,
    query_serial: u32,
    theme: ResolvedTheme,
}

impl WorkspaceView {
    pub fn new(
        cx: &mut Context<Self>,
        hub: Arc<ConnectionHub>,
        sessions: Arc<Mutex<WorkspaceSessionStore>>,
    ) -> Self {
        let theme = theme::read_global(cx).resolved().clone();
        let workspace = cx.entity().downgrade();
        let schema_sidebar = cx.new(|cx| SchemaSidebar::new(cx, workspace));
        Self {
            focus_handle: cx.focus_handle(),
            hub,
            sessions,
            connection_id: None,
            connection_label: "Not connected".into(),
            schema_sidebar,
            tabs: Vec::new(),
            active_tab: 0,
            next_tab_id: 1,
            table_grids: HashMap::new(),
            query_serial: 1,
            theme,
        }
    }

    pub fn set_connection(&mut self, id: ConnectionId, cx: &mut Context<Self>) {
        self.persist_current(cx);
        self.connection_id = Some(id);
        if let Some(profile) = self.hub.get(id) {
            self.connection_label = SharedString::from(profile.name.clone());
        }
        self.schema_sidebar.update(cx, |sidebar, cx| {
            sidebar.set_connection(id, &self.connection_label, cx);
        });
        self.restore_tabs_for_connection(cx);
        cx.notify();
    }

    pub fn open_table_data(&mut self, name: impl Into<SharedString>, cx: &mut Context<Self>) {
        let title = name.into();
        if let Some(ix) = self.tabs.iter().position(|t| {
            t.kind == WorkspaceTabKind::TableData && t.title == title
        }) {
            self.set_active_tab(ix, cx);
        } else {
            self.push_table_tab(title, false, cx);
        }
    }

    pub fn open_structure(&mut self, name: SharedString, cx: &mut Context<Self>) {
        let title = SharedString::from(format!("{name} (structure)"));
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(WorkspaceTab {
            id,
            title,
            kind: WorkspaceTabKind::TableStructure,
            pinned: false,
        });
        self.active_tab = self.tabs.len() - 1;
        self.after_tab_change(cx);
    }

    fn push_table_tab(&mut self, title: SharedString, pinned: bool, cx: &mut Context<Self>) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let grid = cx.new(|cx| DataGrid::new_table(title.clone(), cx));
        self.table_grids.insert(id, grid);
        self.tabs.push(WorkspaceTab {
            id,
            title,
            kind: WorkspaceTabKind::TableData,
            pinned,
        });
        self.active_tab = self.tabs.len() - 1;
        self.after_tab_change(cx);
    }

    fn new_query_tab(&mut self, cx: &mut Context<Self>) {
        let title = SharedString::from(format!("Query {}", self.query_serial));
        self.query_serial += 1;
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(WorkspaceTab {
            id,
            title,
            kind: WorkspaceTabKind::Query,
            pinned: false,
        });
        self.active_tab = self.tabs.len() - 1;
        self.after_tab_change(cx);
    }

    fn close_active_tab(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(self.active_tab) else {
            return;
        };
        if tab.pinned {
            return;
        }
        let id = tab.id;
        self.tabs.remove(self.active_tab);
        self.table_grids.remove(&id);
        if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
            self.active_tab = self.tabs.len() - 1;
        }
        self.after_tab_change(cx);
    }

    fn cycle_tab(&mut self, forward: bool, cx: &mut Context<Self>) {
        if self.tabs.len() < 2 {
            return;
        }
        if forward {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
        } else {
            self.active_tab = self.active_tab.checked_sub(1).unwrap_or(self.tabs.len() - 1);
        }
        self.after_tab_change(cx);
    }

    fn move_active_tab(&mut self, delta: i32, cx: &mut Context<Self>) {
        let len = self.tabs.len();
        if len < 2 {
            return;
        }
        let from = self.active_tab;
        let to = ((from as i32 + delta).rem_euclid(len as i32)) as usize;
        if from == to {
            return;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        self.active_tab = to;
        self.persist_current(cx);
        self.after_tab_change(cx);
    }

    fn set_active_tab(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.tabs.len() {
            self.active_tab = ix;
            self.after_tab_change(cx);
        }
    }

    fn after_tab_change(&mut self, cx: &mut Context<Self>) {
        self.evict_background_tabs(cx);
        if let Some(tab) = self.tabs.get(self.active_tab) {
            if tab.kind == WorkspaceTabKind::TableData {
                if let Some(grid) = self.table_grids.get(&tab.id) {
                    grid.update(cx, |grid, cx| {
                        grid.mark_active();
                        cx.notify();
                    });
                }
            }
        }
        self.persist_current(cx);
        cx.notify();
    }

    fn evict_background_tabs(&self, cx: &mut Context<Self>) {
        let active_id = self.tabs.get(self.active_tab).map(|t| t.id);
        for (tab_id, grid) in &self.table_grids {
            if Some(*tab_id) != active_id {
                grid.update(cx, |grid, _| grid.unload_for_background());
            }
        }
    }

    fn persist_current(&self, cx: &App) {
        let Some(connection_id) = self.connection_id else {
            return;
        };
        let state = WorkspaceTabState {
            active_tab: self.active_tab,
            next_tab_id: self.next_tab_id,
            tabs: self
                .tabs
                .iter()
                .map(|t| SavedWorkspaceTab {
                    title: t.title.to_string(),
                    kind: match t.kind {
                        WorkspaceTabKind::TableData => SavedWorkspaceTabKind::TableData,
                        WorkspaceTabKind::TableStructure => SavedWorkspaceTabKind::TableStructure,
                        WorkspaceTabKind::Query => SavedWorkspaceTabKind::Query,
                    },
                    pinned: t.pinned,
                })
                .collect(),
        };
        if let Ok(mut store) = self.sessions.lock() {
            store.set(connection_id, state);
            let _ = store.save();
        }
        let _ = cx;
    }

    fn restore_tabs_for_connection(&mut self, cx: &mut Context<Self>) {
        self.tabs.clear();
        self.table_grids.clear();
        self.active_tab = 0;
        let Some(connection_id) = self.connection_id else {
            return;
        };
        let snapshot = self
            .sessions
            .lock()
            .ok()
            .and_then(|store| store.get(connection_id).cloned());
        let Some(state) = snapshot else {
            return;
        };
        self.next_tab_id = state.next_tab_id.max(1);
        for saved in state.tabs {
            let title: SharedString = saved.title.into();
            match saved.kind {
                SavedWorkspaceTabKind::TableData => {
                    let id = self.next_tab_id;
                    self.next_tab_id += 1;
                    let grid = cx.new(|cx| DataGrid::new_table(title.clone(), cx));
                    grid.update(cx, |g, _| g.unload_for_background());
                    self.table_grids.insert(id, grid);
                    self.tabs.push(WorkspaceTab {
                        id,
                        title,
                        kind: WorkspaceTabKind::TableData,
                        pinned: saved.pinned,
                    });
                }
                SavedWorkspaceTabKind::TableStructure => {
                    let id = self.next_tab_id;
                    self.next_tab_id += 1;
                    self.tabs.push(WorkspaceTab {
                        id,
                        title,
                        kind: WorkspaceTabKind::TableStructure,
                        pinned: saved.pinned,
                    });
                }
                SavedWorkspaceTabKind::Query => {
                    let id = self.next_tab_id;
                    self.next_tab_id += 1;
                    self.tabs.push(WorkspaceTab {
                        id,
                        title,
                        kind: WorkspaceTabKind::Query,
                        pinned: saved.pinned,
                    });
                }
            }
        }
        if !self.tabs.is_empty() {
            self.active_tab = state.active_tab.min(self.tabs.len() - 1);
            self.after_tab_change(cx);
        }
    }
}

impl Focusable for WorkspaceView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = &self.theme.colors;
        let sidebar = self.schema_sidebar.clone();

        div()
            .id("workspace")
            .track_focus(&self.focus_handle)
            .key_context("WispWorkspace")
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                let mod_cmd = event.keystroke.modifiers.platform;
                let ctrl = event.keystroke.modifiers.control;
                match event.keystroke.key.as_str() {
                    "t" if mod_cmd => this.new_query_tab(cx),
                    "w" if mod_cmd => this.close_active_tab(cx),
                    "tab" if ctrl => this.cycle_tab(!event.keystroke.modifiers.shift, cx),
                    _ => {}
                }
            }))
            .size_full()
            .flex()
            .bg(c.canvas)
            .child(
                div()
                    .w(px(SIDEBAR_WIDTH))
                    .h_full()
                    .flex_none()
                    .child(sidebar),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .bg(c.panel)
                    .child(tab_bar(c, &self.tabs, self.active_tab, cx))
                    .child(main_pane(
                        c,
                        &self.tabs,
                        self.active_tab,
                        &self.table_grids,
                    )),
            )
    }
}

fn tab_bar(
    c: &theme::ResolvedColors,
    tabs: &[WorkspaceTab],
    active: usize,
    cx: &mut Context<WorkspaceView>,
) -> impl IntoElement {
    div()
        .flex_none()
        .flex()
        .gap_1()
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(c.line)
        .bg(c.raise)
        .children(tabs.iter().enumerate().map(|(ix, tab)| {
            let is_active = ix == active;
            let tab_id = tab.id;
            let tab_pinned = tab.pinned;
            let label = if tab.pinned {
                format!("{} (pinned)", tab.title)
            } else {
                tab.title.to_string()
            };
            div()
                .id(("wtab", tab.id))
                .flex()
                .items_center()
                .gap_1()
                .px_2()
                .py_1()
                .rounded_md()
                .text_sm()
                .cursor_pointer()
                .bg(if is_active { c.panel } else { c.raise })
                .border_1()
                .border_color(if is_active { c.line } else { c.line2 })
                .text_color(if is_active { c.ink1 } else { c.ink2 })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_active_tab(ix, cx);
                }))
                .child(label)
                .child(
                    div()
                        .id(("wtab-close", tab.id))
                        .text_xs()
                        .px_1()
                        .rounded_md()
                        .cursor_pointer()
                        .text_color(c.ink3)
                        .hover(|s| s.bg(c.line2))
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(move |_, event: &gpui::MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                let _ = event;
                            }),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if tab_pinned {
                                return;
                            }
                            if let Some(pos) = this.tabs.iter().position(|t| t.id == tab_id) {
                                this.tabs.remove(pos);
                                this.table_grids.remove(&tab_id);
                                if this.active_tab >= this.tabs.len() && !this.tabs.is_empty() {
                                    this.active_tab = this.tabs.len() - 1;
                                }
                                this.after_tab_change(cx);
                            }
                        }))
                        .child("×"),
                )
                .on_mouse_down(
                    gpui::MouseButton::Middle,
                    cx.listener(move |this, _, _, cx| {
                        if tab_pinned {
                            return;
                        }
                        if let Some(pos) = this.tabs.iter().position(|t| t.id == tab_id) {
                            this.tabs.remove(pos);
                            this.table_grids.remove(&tab_id);
                            if this.active_tab >= this.tabs.len() && !this.tabs.is_empty() {
                                this.active_tab = this.tabs.len() - 1;
                            }
                            this.after_tab_change(cx);
                        }
                    }),
                )
                .child(
                    div()
                        .id(("wtab-pin", tab.id))
                        .text_xs()
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(t) = this.tabs.iter_mut().find(|t| t.id == tab_id) {
                                t.pinned = !t.pinned;
                                this.persist_current(cx);
                                cx.notify();
                            }
                        }))
                        .child(if tab.pinned { "•" } else { "○" }),
                )
                .child(
                    div()
                        .id(("wtab-left", tab.id))
                        .text_xs()
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.active_tab == ix {
                                this.move_active_tab(-1, cx);
                            }
                        }))
                        .child("‹"),
                )
                .child(
                    div()
                        .id(("wtab-right", tab.id))
                        .text_xs()
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.active_tab == ix {
                                this.move_active_tab(1, cx);
                            }
                        }))
                        .child("›"),
                )
        }))
}

fn main_pane(
    c: &theme::ResolvedColors,
    tabs: &[WorkspaceTab],
    active: usize,
    grids: &HashMap<u32, Entity<DataGrid>>,
) -> impl IntoElement {
    let Some(tab) = tabs.get(active) else {
        return div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_color(c.ink3)
            .text_sm()
            .child("Open a table from the sidebar.")
            .into_any_element();
    };
    match tab.kind {
        WorkspaceTabKind::TableData => {
            if let Some(grid) = grids.get(&tab.id) {
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_none()
                            .px_2()
                            .py_1()
                            .border_b_1()
                            .border_color(c.line)
                            .text_sm()
                            .text_color(c.ink2)
                            .child(format!("{} · read-only", tab.title)),
                    )
                    .child(div().flex_1().min_h_0().child(grid.clone()))
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child("Grid unavailable.")
                    .into_any_element()
            }
        }
        WorkspaceTabKind::TableStructure | WorkspaceTabKind::Query => div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_color(c.ink3)
            .text_sm()
            .child(format!("{} — coming soon.", tab.title))
            .into_any_element(),
    }
}
