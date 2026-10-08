use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex}, time::Instant};

use gpui::{
    div, prelude::*, px, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Task, Window, WindowBounds,
};
use wisp_core::{
    bridge::DbEventPayload, ConnectionHub, DbCommandPayload, SessionOpenSpec, SessionPhase,
    WorkspaceSessionStore,
};

use crate::connections::{
    ConnectionForm, ConnectionManager, ShellCommand, ShellCommandSender,
};

#[cfg(feature = "ui-gallery")]
use gpui::Entity;

#[cfg(feature = "ui-gallery")]
use crate::components::gallery::ComponentGallery;


use crate::{
    components::toast,
    environment::Environment,
    launch::{AppearanceConfig, SharedSettingsInbox, ShellMetrics, WindowPersistence},
    memory,
    multi_window::{drain_pending_windows, WindowOpenQueue},
    route::Route,
    theme::{self, ResolvedTheme},
    workspace::WorkspaceView,
};

pub struct WispShell {
    route: Route,
    environment: Environment,
    connection_status: SharedString,
    timing_status: SharedString,
    launch_started: Instant,
    first_frame_reported: bool,
    persistence: Rc<RefCell<WindowPersistence>>,
    metrics: Rc<RefCell<ShellMetrics>>,
    appearance: Rc<RefCell<AppearanceConfig>>,
    settings_inbox: Option<SharedSettingsInbox>,
    settings_toast: Option<SharedString>,
    connection_commands: ShellCommandSender,
    connections_hub: Option<Arc<ConnectionHub>>,
    active_connection: Option<wisp_core::ConnectionId>,
    session_open_task: Option<Task<()>>,
    connection_manager: Option<Entity<ConnectionManager>>,
    connection_form: Option<Entity<ConnectionForm>>,
    workspace: Option<Entity<WorkspaceView>>,
    workspace_sessions: Arc<Mutex<WorkspaceSessionStore>>,
    window_open_queue: WindowOpenQueue,
    #[cfg(feature = "ui-gallery")]
    gallery: Entity<ComponentGallery>,
}

impl WispShell {
    pub fn new(
        persistence: Rc<RefCell<WindowPersistence>>,
        metrics: Rc<RefCell<ShellMetrics>>,
        appearance: Rc<RefCell<AppearanceConfig>>,
        settings_inbox: Option<SharedSettingsInbox>,
        connections: Option<Arc<ConnectionHub>>,
        workspace_sessions: Arc<Mutex<WorkspaceSessionStore>>,
        window_open_queue: WindowOpenQueue,
        initial_connection: Option<wisp_core::ConnectionId>,
        #[cfg(feature = "ui-gallery")] gallery: Entity<ComponentGallery>,
        cx: &mut Context<Self>,
    ) -> Self {
        let connection_commands = ShellCommandSender::default();
        let connection_manager = connections.as_ref().map(|hub| {
            let commands = connection_commands.clone();
            cx.new(|cx| ConnectionManager::new(cx, Arc::clone(hub), commands))
        });
        let connection_form = connections.clone().map(|hub| {
            let commands = connection_commands.clone();
            cx.new(|cx| ConnectionForm::new(cx, hub, commands))
        });
        let workspace = connections.clone().map(|hub| {
            cx.new(|cx| {
                WorkspaceView::new(cx, Arc::clone(&hub), Arc::clone(&workspace_sessions))
            })
        });
        let mut shell = Self {
            route: if initial_connection.is_some() {
                Route::Workspace
            } else {
                Route::Connections
            },
            environment: Environment::Local,
            connection_status: "Not connected".into(),
            timing_status: "Ready".into(),
            launch_started: Instant::now(),
            first_frame_reported: false,
            persistence,
            metrics,
            appearance,
            settings_inbox,
            settings_toast: None,
            connection_commands,
            connections_hub: connections.clone(),
            active_connection: None,
            session_open_task: None,
            connection_manager,
            connection_form,
            workspace,
            workspace_sessions,
            window_open_queue,
            #[cfg(feature = "ui-gallery")]
            gallery,
        };
        if let Some(id) = initial_connection {
            shell.open_session(id, cx);
        }
        shell
    }

    fn drain_connection_commands(&mut self, cx: &mut Context<Self>) {
        self.poll_session_events(cx);
        let commands = self.connection_commands.drain();
        if commands.is_empty() {
            return;
        }
        for command in commands {
            match command {
                ShellCommand::Connect(id) => {
                    self.route = Route::Workspace;
                    self.open_session(id, cx);
                }
                ShellCommand::ConnectNewWindow(id) => {
                    self.window_open_queue.request(id);
                }
                ShellCommand::Edit(id) => {
                    self.route = Route::ConnectionForm;
                    if let Some(form) = self.connection_form.clone() {
                        form.update(cx, |form, cx| form.open_edit(id, cx));
                    }
                }
                ShellCommand::NewConnection => {
                    self.route = Route::ConnectionForm;
                    if let Some(form) = self.connection_form.clone() {
                        form.update(cx, |form, cx| form.open_new(cx));
                    }
                }
                ShellCommand::NewConnectionFromDraft(draft) => {
                    self.route = Route::ConnectionForm;
                    if let Some(form) = self.connection_form.clone() {
                        form.update(cx, |form, cx| form.open_with_draft(*draft, cx));
                    }
                }
                ShellCommand::FormSaved { id, connect } => {
                    if connect {
                        self.route = Route::Workspace;
                        self.open_session(id, cx);
                    } else {
                        self.route = Route::Connections;
                    }
                }
                ShellCommand::FormCancelled => {
                    self.route = Route::Connections;
                }
            }
        }
        cx.notify();
    }

    fn drain_settings_inbox(&mut self, cx: &mut Context<Self>) {
        let Some(inbox) = self.settings_inbox.as_ref() else {
            return;
        };
        let Ok(mut guard) = inbox.lock() else {
            return;
        };
        for toast in guard.toasts.drain(..) {
            let message = if let Some(line) = toast.line {
                format!("settings.toml line {line}: {}", toast.message)
            } else {
                toast.message
            };
            self.settings_toast = Some(message.into());
        }
        if let Some(config) = guard.appearance.take() {
            *self.appearance.borrow_mut() = config.clone();
            theme::update_global(cx, |global, cx| {
                global.apply_appearance(
                    config.theme_mode,
                    config.density,
                    theme::Typography {
                        ui_font: config.ui_font,
                        mono_font: config.mono_font,
                    },
                    cx,
                );
            });
            cx.notify();
        }
    }

    fn open_session(&mut self, id: wisp_core::ConnectionId, cx: &mut Context<Self>) {
        self.active_connection = Some(id);
        if let Some(workspace) = self.workspace.clone() {
            workspace.update(cx, |ws, cx| ws.set_connection(id, cx));
        }
        let Some(hub) = self.connections_hub.as_ref() else {
            self.connection_status = "Not connected".into();
            return;
        };
        let Some(profile) = hub.get(id) else {
            self.connection_status = "Connection not found".into();
            return;
        };
        let _ = hub.touch_connect(id);
        self.environment = Environment::from_tag(&profile.env_tag);
        self.connection_status = format!("Connecting to {}…", profile.name).into();
        self.timing_status = "Opening session".into();
        let spec = SessionOpenSpec {
            profile,
            secrets: hub.session_secrets(id),
        };
        let bridge = crate::bridge::db_bridge(cx);
        let (_id, _cancel, task) = crate::bridge::spawn_db(
            cx,
            &bridge,
            DbCommandPayload::SessionOpen(spec),
            move |this, cx, result| {
                this.session_open_task = None;
                match result {
                    Ok(DbEventPayload::SessionOpen(Ok(snap))) => {
                        this.apply_session_snapshot(snap, cx);
                    }
                    Ok(DbEventPayload::SessionOpen(Err(err))) => {
                        this.connection_status =
                            format!("Connection failed — {}", err).into();
                        this.timing_status = "Failed".into();
                    }
                    Ok(_) | Err(_) => {
                        this.connection_status = "Connection failed".into();
                        this.timing_status = "Failed".into();
                    }
                }
                cx.notify();
            },
        );
        self.session_open_task = Some(task);
    }

    fn apply_session_snapshot(
        &mut self,
        snap: wisp_core::SessionSnapshot,
        cx: &mut Context<Self>,
    ) {
        let version = snap
            .version
            .as_deref()
            .unwrap_or("database");
        self.connection_status = format!("{} — {}", snap.name, snap.status_label()).into();
        self.timing_status = version.into();
        if let Some(id) = self.active_connection {
            if let Some(workspace) = self.workspace.clone() {
                workspace.update(cx, |ws, cx| ws.set_connection(id, cx));
            }
        }
    }

    fn poll_session_events(&mut self, cx: &mut Context<Self>) {
        let Some(active) = self.active_connection else {
            return;
        };
        let bridge = crate::bridge::db_bridge(cx);
        while let Some(event) = bridge.poll_session_event() {
            if event.connection_id != active {
                continue;
            }
            let label = match event.phase {
                SessionPhase::Connecting => "Connecting…",
                SessionPhase::Ready => "Connected",
                SessionPhase::Busy => "Busy",
                SessionPhase::Reconnecting => "Reconnecting…",
                SessionPhase::Failed => "Connection failed",
                SessionPhase::Closed => "Closed",
            };
            if let Some(hub) = self.connections_hub.as_ref() {
                if let Some(profile) = hub.get(active) {
                    self.connection_status =
                        format!("{} — {label}", profile.name).into();
                }
            }
            if let Some(detail) = event.detail {
                self.timing_status = detail.into();
            } else if event.phase == SessionPhase::Ready {
                self.timing_status = "Ready".into();
            }
            cx.notify();
        }
    }

    fn report_first_frame(&mut self) {
        if self.first_frame_reported {
            return;
        }
        self.first_frame_reported = true;
        let ms = self.launch_started.elapsed().as_millis();
        self.metrics.borrow_mut().first_frame_ms = Some(ms);
        if std::env::var_os("WISP_LAUNCH_METRIC").is_some() {
            eprintln!("wisp-first-frame-ms: {ms}");
        }
    }
}

impl Render for WispShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.report_first_frame();
        self.drain_settings_inbox(cx);
        self.drain_connection_commands(cx);
        drain_pending_windows(cx);
        capture_window_state(window, &self.persistence);

        let theme = theme::read_global(cx).resolved().clone();
        let colors = &theme.colors;
        let env_color = self.environment.color(&colors.env);
        let env_on_color = self.environment.on_color(&colors.env);
        let production = self.environment.is_production();
        let route = self.route;
        let title = route.title();
        let env_label = self.environment.label();
        let memory_mb = memory::memory_megabytes();
        let memory_pct = (memory_mb / 40.0).clamp(0.0, 1.0);
        let connection = self.connection_status.clone();
        let timing = crate::grid::grid_status(cx)
            .take_line()
            .map(SharedString::from)
            .unwrap_or_else(|| self.timing_status.clone());
        let status_h = theme.density.status_bar_height();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.window)
            .text_color(colors.ink1)
            .text_size(theme.ui_font_size)
            .font(theme.typography.gpui_ui_font())
            .child(title_bar(
                cx,
                &theme,
                title,
                env_label,
                env_color,
                env_on_color,
                production,
                route,
                self.appearance.clone(),
            ))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(render_route(self, route, &theme)),
            )
            .child(status_bar(
                &theme, status_h, env_color, connection, timing, memory_mb, memory_pct,
            ))
            .children(self.settings_toast.as_ref().map(|message| {
                toast::toast(&theme, message.clone())
            }))
    }
}

#[allow(unused_variables)]
fn render_route(shell: &WispShell, route: Route, theme: &ResolvedTheme) -> impl IntoElement {
    let c = &theme.colors;
    match route {
        #[cfg(feature = "ui-gallery")]
        Route::Gallery => div().size_full().child(shell.gallery.clone()),
        Route::Connections => {
            if let Some(manager) = &shell.connection_manager {
                div().size_full().child(manager.clone())
            } else {
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .bg(c.canvas)
                    .child(
                        div()
                            .text_sm()
                            .text_color(c.ink3)
                            .child("Connection store unavailable."),
                    )
            }
        }
        Route::ConnectionForm => {
            if let Some(form) = &shell.connection_form {
                div().size_full().child(form.clone())
            } else {
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(c.canvas)
                    .child(
                        div()
                            .text_sm()
                            .text_color(c.ink3)
                            .child("Connection store unavailable."),
                    )
            }
        }
        Route::Workspace => {
            if let Some(workspace) = &shell.workspace {
                div().size_full().child(workspace.clone())
            } else {
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(c.canvas)
                    .child(
                        div()
                            .text_sm()
                            .text_color(c.ink3)
                            .child("Connection store unavailable."),
                    )
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn title_bar(
    cx: &mut Context<WispShell>,
    theme: &ResolvedTheme,
    title: &str,
    env_label: &str,
    env_color: gpui::Rgba,
    env_on_color: gpui::Rgba,
    production: bool,
    route: Route,
    appearance: Rc<RefCell<AppearanceConfig>>,
) -> impl IntoElement {
    let c = &theme.colors;
    div()
        .flex()
        .flex_col()
        .flex_none()
        .child(
            div()
                .h(px(40.0))
                .flex()
                .items_center()
                .px_3()
                .gap_2()
                .bg(c.sidebar)
                .border_b_1()
                .border_color(c.line)
                .when(cfg!(target_os = "macos"), |bar| bar.pl(px(72.0)))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .justify_center()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .text_color(c.ink2)
                                .child(format!("Wisp — {title}")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(if production {
                                    gpui::FontWeight::BOLD
                                } else {
                                    gpui::FontWeight::SEMIBOLD
                                })
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .text_color(env_on_color)
                                .bg(env_color)
                                .when(production, |badge| {
                                    badge.border_2().border_color(gpui::rgb(0xffffff))
                                })
                                .child(if production {
                                    format!("PRODUCTION · {env_label}")
                                } else {
                                    env_label.to_string()
                                }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(theme_toggle(cx, c, appearance))
                        .children(title_nav_buttons(cx, c, route))
                ),
        )
        .child(
            div()
                .h(px(if production { 4.0 } else { 2.0 }))
                .w_full()
                .bg(env_color),
        )
}

fn title_nav_buttons(
    cx: &mut Context<WispShell>,
    c: &theme::ResolvedColors,
    route: Route,
) -> Vec<gpui::AnyElement> {
    let mut buttons = Vec::new();
    #[cfg(feature = "ui-gallery")]
    if route != Route::Gallery {
        buttons.push(
            nav_button(cx, c, "nav-gallery", "Gallery", |this| {
                this.route = Route::Gallery;
            })
            .into_any_element(),
        );
    }
    buttons.push(
        if route == Route::Connections {
            nav_button(cx, c, "new-connection", "New connection", |this| {
                this.route = Route::ConnectionForm;
            })
        } else {
            nav_button(cx, c, "back-connections", "Back", |this| {
                this.route = Route::Connections;
            })
        }
        .into_any_element(),
    );
    buttons
}

fn theme_toggle(
    cx: &mut Context<WispShell>,
    c: &theme::ResolvedColors,
    appearance: Rc<RefCell<AppearanceConfig>>,
) -> impl IntoElement {
    div()
        .id("theme-toggle")
        .text_sm()
        .px_2()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(c.line)
        .bg(c.panel)
        .cursor_pointer()
        .hover(|s| s.bg(c.raise))
        .child("Theme")
        .on_click(cx.listener(move |_, _, _window, cx| {
            let appearance = appearance.clone();
            theme::update_global(cx, |global, cx| {
                global.toggle_light_dark(cx);
                appearance.borrow_mut().theme_mode = global.mode;
            });
            cx.notify();
        }))
}

fn nav_button(
    cx: &mut Context<WispShell>,
    c: &theme::ResolvedColors,
    id: &'static str,
    label: &'static str,
    on_nav: fn(&mut WispShell),
) -> impl IntoElement {
    div()
        .id(id)
        .text_sm()
        .px_2()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(c.line)
        .bg(c.panel)
        .cursor_pointer()
        .hover(|s| s.bg(c.sidebar))
        .child(label)
        .on_click(cx.listener(move |this, _, _window, cx| {
            on_nav(this);
            cx.notify();
        }))
}

fn status_bar(
    theme: &ResolvedTheme,
    height: gpui::Pixels,
    env_color: gpui::Rgba,
    connection: SharedString,
    timing: SharedString,
    memory_mb: f64,
    memory_pct: f64,
) -> impl IntoElement {
    let c = &theme.colors;
    div()
        .flex_none()
        .h(height)
        .flex()
        .items_center()
        .px_3()
        .gap_4()
        .bg(c.sidebar)
        .border_t_1()
        .border_color(c.line)
        .text_xs()
        .text_color(c.ink2)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().size(px(8.0)).rounded_full().bg(env_color))
                .child(connection),
        )
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(format!("{memory_mb:.1} MB"))
                        .child(
                            div()
                                .w(px(54.0))
                                .h(px(6.0))
                                .rounded_sm()
                                .bg(c.line)
                                .overflow_hidden()
                                .child(
                                    div()
                                        .h_full()
                                        .w(gpui::Length::Definite(gpui::relative(
                                            memory_pct as f32,
                                        )))
                                        .bg(env_color),
                                ),
                        ),
                )
                .child(timing),
        )
}

fn capture_window_state(window: &Window, persistence: &Rc<RefCell<WindowPersistence>>) {
    let bounds = window.window_bounds();
    let mut next = persistence.borrow().clone();
    match bounds {
        WindowBounds::Maximized(restore) => {
            next.maximized = true;
            next.geometry = Some(bounds_to_geometry(restore));
        }
        WindowBounds::Fullscreen(restore) => {
            next.maximized = false;
            next.geometry = Some(bounds_to_geometry(restore));
        }
        WindowBounds::Windowed(b) => {
            next.maximized = false;
            next.geometry = Some(bounds_to_geometry(b));
        }
    }
    *persistence.borrow_mut() = next;
}

fn bounds_to_geometry(b: gpui::Bounds<gpui::Pixels>) -> crate::launch::WindowGeometry {
    crate::launch::WindowGeometry {
        width: f32::from(b.size.width),
        height: f32::from(b.size.height),
        x: f32::from(b.origin.x),
        y: f32::from(b.origin.y),
    }
}
