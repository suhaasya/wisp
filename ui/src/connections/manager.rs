//! Welcome screen: saved connections grouped by folder (LUM-015).

#![allow(clippy::too_many_arguments)]

use std::{rc::Rc, sync::Arc};

use gpui::{
    div, prelude::*, px, uniform_list, ClickEvent, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement,
    Styled, UniformListScrollHandle, Window,
};
use wisp_core::{
    parse_connection_paste, ConnectionCardView, ConnectionEngineKind, ConnectionHub, ConnectionId,
    ConnectionsView, EnvFilter, RailSelection,
};

use crate::{
    components::{
        button::{button, ButtonVariant},
        overlay::{modal_centered, modal_panel, overlay_close},
        text_input::{TextInput, TextInputKind},
    },
    connections::{
        actions::{NewConnection, PasteConnectionUrl},
        env::{env_edge_colour, parse_edge_colour},
        shell_commands::{ShellCommand, ShellCommandSender},
    },
    theme::{self, ResolvedTheme},
};

const RAIL_WIDTH: f32 = 210.0;
const CARD_ROW_HEIGHT: f32 = 118.0;
const GRID_COLS: usize = 3;
const VIRTUALIZE_THRESHOLD: usize = 60;

#[derive(Clone)]
struct ConnectionDrag {
    connection_id: ConnectionId,
}

struct DragPreview {
    label: SharedString,
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(gpui::rgb(0xffffff))
            .child(self.label.clone())
    }
}

pub struct ConnectionManager {
    focus_handle: FocusHandle,
    hub: Arc<ConnectionHub>,
    commands: ShellCommandSender,
    search_input: Entity<TextInput>,
    rail: RailSelection,
    selected_index: usize,
    scroll: UniformListScrollHandle,
    pending_delete: Option<(ConnectionId, SharedString)>,
    theme: ResolvedTheme,
}

impl ConnectionManager {
    pub fn new(
        cx: &mut Context<Self>,
        hub: Arc<ConnectionHub>,
        commands: ShellCommandSender,
    ) -> Self {
        let theme = theme::read_global(cx).resolved().clone();
        Self {
            focus_handle: cx.focus_handle(),
            hub,
            commands,
            search_input: cx.new(|cx| {
                TextInput::new(
                    cx,
                    "Search connections",
                    TextInputKind::SingleLine,
                    theme.clone(),
                )
            }),
            rail: RailSelection::All,
            selected_index: 0,
            scroll: UniformListScrollHandle::default(),
            pending_delete: None,
            theme,
        }
    }

    fn current_view(&self, cx: &gpui::App) -> ConnectionsView {
        let search = self.search_input.read(cx).content().to_string();
        let folders = self.hub.folders();
        let profiles = self.hub.profiles();
        ConnectionsView::build(&folders, &profiles, self.rail, &search)
    }

    fn connect(&self, id: ConnectionId) {
        let _ = self.hub.touch_connect(id);
        self.commands.push(ShellCommand::Connect(id));
    }

    fn clamp_selection(&mut self, count: usize) {
        if count == 0 {
            self.selected_index = 0;
        } else {
            self.selected_index = self.selected_index.min(count - 1);
        }
    }

    fn edit(&self, id: ConnectionId) {
        self.commands.push(ShellCommand::Edit(id));
    }

    fn new_connection(&self) {
        self.commands.push(ShellCommand::NewConnection);
    }

    fn paste_connection_url(&self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        if let Ok(draft) = parse_connection_paste(&text) {
            self.commands
                .push(ShellCommand::NewConnectionFromDraft(Box::new(draft)));
        }
    }
}

impl Focusable for ConnectionManager {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ConnectionManager {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.theme = theme::read_global(cx).resolved().clone();
        let view = self.current_view(cx);
        self.clamp_selection(view.flat_cards.len());

        let c = &self.theme.colors;
        let hub = self.hub.clone();

        div()
            .size_full()
            .flex()
            .bg(c.canvas)
            .text_color(c.ink1)
            .text_size(self.theme.ui_font_size)
            .font(self.theme.typography.gpui_ui_font())
            .track_focus(&self.focus_handle)
            .key_context("WispConnections")
            .on_action(cx.listener(|this, _: &NewConnection, _, _| {
                this.new_connection();
            }))
            .on_action(cx.listener(|this, _: &PasteConnectionUrl, _, cx| {
                this.paste_connection_url(cx);
            }))
            .on_key_down(cx.listener(
                |this, event: &gpui::KeyDownEvent, _, cx| {
                    let key = event.keystroke.key.as_str();
                    let count = this.current_view(cx).flat_cards.len();
                    match key {
                        "down" | "j" => {
                            this.select_next(count);
                            cx.notify();
                        }
                        "up" | "k" => {
                            this.select_prev();
                            cx.notify();
                        }
                        "enter" => {
                            if let Some(card) = this.current_view(cx).flat_cards.get(this.selected_index) {
                                this.connect(card.id);
                            }
                        }
                        _ => {}
                    }
                },
            ))
            .child(rail(
                cx,
                &self.theme,
                &view,
                self.rail,
                hub.clone(),
                |this, rail, cx| {
                    this.rail = rail;
                    this.selected_index = 0;
                    cx.notify();
                },
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(home_header(
                        cx,
                        &self.theme,
                        self.search_input.clone(),
                        |this, _, _| this.new_connection(),
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .overflow_hidden()
                            .px_7()
                            .py_5()
                            .child(home_body(
                                cx,
                                &self.theme,
                                &view,
                                self.selected_index,
                                self.scroll.clone(),
                                hub,
                                self.commands.clone(),
                                |this, id, cx| {
                                    this.connect(id);
                                    cx.notify();
                                },
                                |this, id, cx| {
                                    this.edit(id);
                                    cx.notify();
                                },
                                |this, id, name, cx| {
                                    this.pending_delete = Some((id, name));
                                    cx.notify();
                                },
                                |this, index, cx| {
                                    this.selected_index = index;
                                    cx.notify();
                                },
                            )),
                    ),
            )
            .children(delete_confirm(
                cx,
                &self.theme,
                self.pending_delete.clone(),
                |this, _, cx| {
                    this.pending_delete = None;
                    cx.notify();
                },
                |this, cx| {
                    if let Some((id, _)) = this.pending_delete.take() {
                        let _ = this.hub.delete(id);
                        cx.notify();
                    }
                },
            ))
    }
}

impl ConnectionManager {
    fn select_next(&mut self, count: usize) {
        if count == 0 {
            self.selected_index = 0;
        } else {
            self.selected_index = (self.selected_index + 1).min(count - 1);
        }
    }

    fn select_prev(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }
}

fn home_header<V: 'static>(
    cx: &mut Context<V>,
    theme: &ResolvedTheme,
    search: Entity<TextInput>,
    on_new: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let c = &theme.colors;
    div()
        .flex()
        .items_center()
        .gap_2()
        .px_7()
        .pt_5()
        .pb_1()
        .child(
            div()
                .flex_1()
                .max_w(px(360.))
                .child(search.clone()),
        )
        .child(button(
            cx,
            "conn-new",
            "New connection",
            ButtonVariant::Primary,
            theme,
            on_new,
        ))
        .child(
            div()
                .text_sm()
                .text_color(c.ink3)
                .child("Import"),
        )
}

fn home_body<V: 'static>(
    cx: &mut Context<V>,
    theme: &ResolvedTheme,
    view: &ConnectionsView,
    selected_index: usize,
    scroll: UniformListScrollHandle,
    hub: Arc<ConnectionHub>,
    commands: ShellCommandSender,
    on_connect: impl Fn(&mut V, ConnectionId, &mut Context<V>) + 'static + Clone,
    on_edit: impl Fn(&mut V, ConnectionId, &mut Context<V>) + 'static + Clone,
    on_delete: impl Fn(&mut V, ConnectionId, SharedString, &mut Context<V>) + 'static + Clone,
    on_select: impl Fn(&mut V, usize, &mut Context<V>) + 'static + Clone,
) -> impl IntoElement {
    let c = &theme.colors;
    div().flex().flex_col().gap_3().children(if view.total_profiles == 0 {
        vec![div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .py_16()
            .gap_2()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("No connections yet"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(c.ink3)
                    .child("Create a connection or paste a URL to get started."),
            )
            .into_any_element()]
    } else {
        let mut nodes = vec![
            div()
                .text_xs()
                .text_color(c.ink3)
                .child("Paste a connection URL with ⌘V to open a pre-filled form.")
                .into_any_element(),
        ];
        if view.flat_cards.is_empty() {
            nodes.push(
                div()
                    .text_sm()
                    .text_color(c.ink3)
                    .child("No connections match your search or filters.")
                    .into_any_element(),
            );
        } else if view.flat_cards.len() > VIRTUALIZE_THRESHOLD {
            nodes.push(
                virtualized_grid(
                    cx,
                    theme,
                    &view.flat_cards,
                    selected_index,
                    scroll,
                    hub,
                    commands.clone(),
                    on_connect.clone(),
                    on_edit.clone(),
                    on_delete.clone(),
                    on_select.clone(),
                )
                .into_any_element(),
            );
        } else {
            nodes.push(
                grouped_grid(
                    cx,
                    theme,
                    view,
                    selected_index,
                    hub,
                    commands,
                    on_connect,
                    on_edit,
                    on_delete,
                    on_select,
                )
                .into_any_element(),
            );
        }
        nodes
    })
}

fn grouped_grid<V: 'static>(
    cx: &mut Context<V>,
    theme: &ResolvedTheme,
    view: &ConnectionsView,
    selected_index: usize,
    hub: Arc<ConnectionHub>,
    commands: ShellCommandSender,
    on_connect: impl Fn(&mut V, ConnectionId, &mut Context<V>) + 'static + Clone,
    on_edit: impl Fn(&mut V, ConnectionId, &mut Context<V>) + 'static + Clone,
    on_delete: impl Fn(&mut V, ConnectionId, SharedString, &mut Context<V>) + 'static + Clone,
    on_select: impl Fn(&mut V, usize, &mut Context<V>) + 'static + Clone,
) -> impl IntoElement {
    let mut offset = 0;
    div().flex().flex_col().gap_4().children(view.groups.iter().filter_map(|group| {
        if group.cards.is_empty() {
            return None;
        }
        let section = div()
            .flex()
            .flex_col()
            .gap_3()
            .when(!group.title.is_empty(), |el| {
                el.child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(group.title.clone()),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .children(group.cards.iter().enumerate().map(|(i, card)| {
                        let ix = offset + i;
                        connection_card(
                            cx,
                            theme,
                            card,
                            ix == selected_index,
                            hub.clone(),
                            commands.clone(),
                            on_connect.clone(),
                            on_edit.clone(),
                            on_delete.clone(),
                            on_select.clone(),
                            ix,
                        )
                    })),
            );
        offset += group.cards.len();
        Some(section)
    }))
}

fn virtualized_grid<V: 'static>(
    cx: &mut Context<V>,
    theme: &ResolvedTheme,
    cards: &[ConnectionCardView],
    selected_index: usize,
    scroll: UniformListScrollHandle,
    hub: Arc<ConnectionHub>,
    commands: ShellCommandSender,
    on_connect: impl Fn(&mut V, ConnectionId, &mut Context<V>) + 'static + Clone,
    on_edit: impl Fn(&mut V, ConnectionId, &mut Context<V>) + 'static + Clone,
    on_delete: impl Fn(&mut V, ConnectionId, SharedString, &mut Context<V>) + 'static + Clone,
    on_select: impl Fn(&mut V, usize, &mut Context<V>) + 'static + Clone,
) -> impl IntoElement {
    let cards = Rc::new(cards.to_vec());
    let row_count = cards.len().div_ceil(GRID_COLS);
    let theme = theme.clone();

    uniform_list(
        "connection-grid",
        row_count,
        cx.processor(move |_this, range, _window, cx| {
            let mut rows = Vec::new();
            for row in range {
                rows.push(
                    div()
                        .h(px(CARD_ROW_HEIGHT))
                        .flex()
                        .gap_3()
                        .children((0..GRID_COLS).filter_map(|col| {
                            let ix = row * GRID_COLS + col;
                            let card = cards.get(ix)?;
                            Some(connection_card(
                                cx,
                                &theme,
                                card,
                                ix == selected_index,
                                hub.clone(),
                                commands.clone(),
                                on_connect.clone(),
                                on_edit.clone(),
                                on_delete.clone(),
                                on_select.clone(),
                                ix,
                            ))
                        })),
                );
            }
            rows
        }),
    )
    .h_full()
    .track_scroll(&scroll)
}

fn connection_card<V: 'static>(
    cx: &mut Context<V>,
    theme: &ResolvedTheme,
    card: &ConnectionCardView,
    selected: bool,
    hub: Arc<ConnectionHub>,
    commands: ShellCommandSender,
    on_connect: impl Fn(&mut V, ConnectionId, &mut Context<V>) + 'static + Clone,
    on_edit: impl Fn(&mut V, ConnectionId, &mut Context<V>) + 'static + Clone,
    on_delete: impl Fn(&mut V, ConnectionId, SharedString, &mut Context<V>) + 'static + Clone,
    on_select: impl Fn(&mut V, usize, &mut Context<V>) + 'static + Clone,
    index: usize,
) -> impl IntoElement {
    let c = &theme.colors;
    let edge = card
        .edge_colour
        .as_deref()
        .and_then(parse_edge_colour)
        .unwrap_or_else(|| env_edge_colour(&card.env_tag, &c.env));
    let id = card.id;
    let name = SharedString::from(card.name.clone());
    let connect = on_connect.clone();
    let edit = on_edit.clone();
    let delete = on_delete.clone();
    let select = on_select.clone();
    let commands_window = commands.clone();
    let commands_shift = commands.clone();
    let drag_id = id;
    let (eng_label, eng_bg) = engine_badge(card.engine);

    div()
        .id(("conn-card", index as u32))
        .relative()
        .w(px(250.))
        .flex_1()
        .min_w(px(220.))
        .max_w(px(320.))
        .cursor_pointer()
        .rounded_lg()
        .border_1()
        .border_color(if selected { c.ink3 } else { c.line })
        .bg(c.panel)
        .hover(|s| s.border_color(c.ink3))
        .when(selected, |s| s.bg(c.accent_soft))
        .child(
            div()
                .absolute()
                .left_0()
                .top(px(10.))
                .bottom(px(10.))
                .w(px(4.))
                .rounded_r_md()
                .bg(edge),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .p_3()
                .pl_4()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(26.))
                                .h(px(26.))
                                .rounded_md()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(gpui::rgb(0xffffff))
                                .bg(eng_bg)
                                .child(eng_label),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .overflow_hidden()
                                .child(card.name.clone()),
                        )
                        .children(card.last_used_label.as_ref().map(|when| {
                            div().text_xs().text_color(c.ink3).ml_auto().child(when.clone())
                        }))
                        .when(card.last_used_label.is_none(), |row| {
                            row.child(
                                div()
                                    .text_xs()
                                    .text_color(c.ink3)
                                    .ml_auto()
                                    .child(card.env_label.clone()),
                            )
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .font(theme.typography.gpui_mono_font())
                        .text_color(c.ink2)
                        .overflow_hidden()
                        .child(card.host_line.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .children(card.badges.iter().map(|badge| {
                            div()
                                .text_xs()
                                .px_1p5()
                                .rounded_md()
                                .border_1()
                                .border_color(c.line)
                                .bg(c.raise)
                                .text_color(c.ink2)
                                .child(badge.label.clone())
                        })),
                )
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .mt_1()
                        .child(mini_action(cx, c, "Window", {
                            let commands = commands_window.clone();
                            let hub = hub.clone();
                            move |_, _, _| {
                                let _ = hub.touch_connect(id);
                                commands.push(ShellCommand::ConnectNewWindow(id));
                            }
                        }))
                        .child(mini_action(cx, c, "Edit", {
                            let edit = edit.clone();
                            move |this, _, cx| edit(this, id, cx)
                        }))
                        .child(mini_action(cx, c, "Duplicate", {
                            let hub = hub.clone();
                            move |_this, _, cx| {
                                let _ = hub.duplicate(id);
                                cx.notify();
                            }
                        }))
                        .child(mini_action(cx, c, "Delete", {
                            let delete = delete.clone();
                            let name = name.clone();
                            move |this, _, cx| delete(this, id, name.clone(), cx)
                        })),
                ),
        )
        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
            if event.click_count() >= 2 {
                if event.modifiers().shift {
                    let _ = hub.clone().touch_connect(id);
                    commands_shift.push(ShellCommand::ConnectNewWindow(id));
                } else {
                    connect(this, id, cx);
                }
            } else {
                select(this, index, cx);
            }
        }))
        .on_drag(
            ConnectionDrag {
                connection_id: drag_id,
            },
            {
                let label = SharedString::from(card.name.clone());
                move |_drag, _offset, _window, cx| {
                    cx.new(|_| DragPreview {
                        label: label.clone(),
                    })
                }
            },
        )
}

fn mini_action<V: 'static>(
    cx: &mut Context<V>,
    c: &theme::ResolvedColors,
    label: &'static str,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("mini-{label}")))
        .text_xs()
        .px_1p5()
        .py_0p5()
        .rounded_md()
        .cursor_pointer()
        .text_color(c.ink2)
        .hover(|s| s.bg(c.raise))
        .child(label)
        .on_click(cx.listener(
            move |view, _: &ClickEvent, window, cx| on_click(view, window, cx),
        ))
}

fn engine_badge(engine: ConnectionEngineKind) -> (&'static str, gpui::Rgba) {
    match engine {
        ConnectionEngineKind::PostgreSql => ("PG", gpui::rgb(0x336791)),
        ConnectionEngineKind::MySql => ("MY", gpui::rgb(0xD1822B)),
        ConnectionEngineKind::MariaDb => ("MD", gpui::rgb(0x7A5B45)),
    }
}

fn rail<V: 'static>(
    cx: &mut Context<V>,
    theme: &ResolvedTheme,
    view: &ConnectionsView,
    selected: RailSelection,
    hub: Arc<ConnectionHub>,
    on_select: impl Fn(&mut V, RailSelection, &mut Context<V>) + 'static + Clone,
) -> impl IntoElement {
    let c = &theme.colors;
    div()
        .w(px(RAIL_WIDTH))
        .flex_none()
        .flex()
        .flex_col()
        .bg(c.sidebar)
        .border_r_1()
        .border_color(c.line)
        .px_2()
        .py_3()
        .gap_0p5()
        .child(rail_heading("Folders"))
        .child(rail_item(
            cx,
            theme,
            "All connections",
            view.all_count,
            selected == RailSelection::All,
            on_select.clone(),
            RailSelection::All,
            None,
        ))
        .children(view.folder_rows.iter().map(|(id, name, _, count)| {
            rail_item(
                cx,
                theme,
                name,
                *count,
                selected == RailSelection::Folder(*id),
                on_select.clone(),
                RailSelection::Folder(*id),
                Some((hub.clone(), *id)),
            )
        }))
        .child(rail_heading("Environment"))
        .children(view.env_counts.iter().map(|(env, count)| {
            let label = env.label();
            rail_env_item(
                cx,
                theme,
                label,
                *count,
                selected == RailSelection::Environment(*env),
                on_select.clone(),
                RailSelection::Environment(*env),
                *env,
            )
        }))
}

fn rail_heading(label: &'static str) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(gpui::rgb(0x6E7788))
        .font_weight(gpui::FontWeight::MEDIUM)
        .px_2()
        .mt_2()
        .mb_1()
        .child(label)
}

fn rail_item<V: 'static>(
    cx: &mut Context<V>,
    theme: &ResolvedTheme,
    label: &str,
    count: usize,
    active: bool,
    on_select: impl Fn(&mut V, RailSelection, &mut Context<V>) + 'static + Clone,
    rail: RailSelection,
    drop_target: Option<(Arc<ConnectionHub>, ConnectionId)>,
) -> impl IntoElement {
    let c = &theme.colors;
    let label = label.to_string();
    let click_select = on_select.clone();
    let mut row = div()
        .id(SharedString::from(format!("rail-item-{label}")))
        .flex()
        .items_center()
        .justify_between()
        .px_2()
        .py_1p5()
        .rounded_md()
        .cursor_pointer()
        .bg(if active { c.accent_soft } else { c.sidebar })
        .on_click(cx.listener(move |this, _, _, cx| click_select(this, rail, cx)));
    if let Some((hub, folder_id)) = drop_target {
        row = row.on_drop(cx.listener(move |this, drag: &ConnectionDrag, _, cx| {
            let _ = hub.move_to_folder(drag.connection_id, Some(folder_id));
            on_select(this, RailSelection::Folder(folder_id), cx);
        }));
    }
    row.child(div().text_sm().child(label))
        .child(
            div()
                .text_xs()
                .text_color(c.ink3)
                .child(count.to_string()),
        )
}

fn rail_env_item<V: 'static>(
    cx: &mut Context<V>,
    theme: &ResolvedTheme,
    label: &'static str,
    count: usize,
    active: bool,
    on_select: impl Fn(&mut V, RailSelection, &mut Context<V>) + 'static + Clone,
    rail: RailSelection,
    env: EnvFilter,
) -> impl IntoElement {
    let c = &theme.colors;
    let dot = env_edge_colour(
        &match env {
            EnvFilter::Production => wisp_core::EnvironmentTag::Production,
            EnvFilter::Staging => wisp_core::EnvironmentTag::Staging,
            EnvFilter::Development => wisp_core::EnvironmentTag::Development,
            EnvFilter::Local => wisp_core::EnvironmentTag::Custom("local".into()),
        },
        &c.env,
    );
    div()
        .id(SharedString::from(format!("rail-env-{label}")))
        .flex()
        .items_center()
        .justify_between()
        .px_2()
        .py_1p5()
        .rounded_md()
        .cursor_pointer()
        .bg(if active { c.accent_soft } else { c.sidebar })
        .on_click(cx.listener(move |this, _, _, cx| on_select(this, rail, cx)))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().size(px(8.)).rounded_full().bg(dot))
                .child(div().text_sm().child(label)),
        )
        .child(
            div()
                .text_xs()
                .text_color(c.ink3)
                .child(count.to_string()),
        )
}

fn delete_confirm<V: 'static>(
    cx: &mut Context<V>,
    theme: &ResolvedTheme,
    pending: Option<(ConnectionId, SharedString)>,
    on_cancel: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static + Clone,
    on_confirm: impl Fn(&mut V, &mut Context<V>) + 'static + Clone,
) -> Option<impl IntoElement> {
    let (_id, name) = pending?;
    let focus = cx.focus_handle();
    let cancel = on_cancel.clone();
    Some(modal_centered(
        modal_panel(
            theme,
            "Delete connection?",
            &focus,
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .child(format!(
                            "Remove “{name}” from saved connections? Secrets for this profile will be deleted."
                        )),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(button(
                            cx,
                            "delete-cancel",
                            "Cancel",
                            ButtonVariant::Default,
                            theme,
                            move |this, window, cx| cancel(this, window, cx),
                        ))
                        .child(button(
                            cx,
                            "delete-confirm",
                            "Delete",
                            ButtonVariant::Warning,
                            theme,
                            move |this, _, cx| on_confirm(this, cx),
                        )),
                ),
            overlay_close(cx, theme, on_cancel),
        ),
    ))
}
