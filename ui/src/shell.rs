use std::{cell::RefCell, rc::Rc, time::Instant};

use gpui::{
    div, point, prelude::*, px, rgb, Context, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Window, WindowBounds,
};

use crate::{
    environment::Environment,
    launch::{ShellMetrics, WindowPersistence},
    memory,
    route::Route,
    theme::Theme,
};

pub struct WispShell {
    route: Route,
    environment: Environment,
    theme: Theme,
    connection_status: SharedString,
    timing_status: SharedString,
    launch_started: Instant,
    first_frame_reported: bool,
    persistence: Rc<RefCell<WindowPersistence>>,
    metrics: Rc<RefCell<ShellMetrics>>,
}

impl WispShell {
    pub fn new(
        persistence: Rc<RefCell<WindowPersistence>>,
        metrics: Rc<RefCell<ShellMetrics>>,
    ) -> Self {
        Self {
            route: Route::Connections,
            environment: Environment::Local,
            theme: Theme::light(),
            connection_status: "Not connected".into(),
            timing_status: "Ready".into(),
            launch_started: Instant::now(),
            first_frame_reported: false,
            persistence,
            metrics,
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
        capture_window_state(window, &self.persistence);

        let theme = self.theme.clone();
        let env_color = self.environment.color();
        let route = self.route;
        let title = route.title();
        let env_label = self.environment.label();
        let memory_mb = memory::memory_megabytes();
        let memory_pct = (memory_mb / 40.0).clamp(0.0, 1.0);
        let connection = self.connection_status.clone();
        let timing = self.timing_status.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.window)
            .text_color(theme.ink)
            .child(title_bar(cx, &theme, title, env_label, env_color, route))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(render_route(route, &theme)),
            )
            .child(status_bar(
                &theme, env_color, connection, timing, memory_mb, memory_pct,
            ))
    }
}

fn render_route(route: Route, theme: &Theme) -> impl IntoElement {
    match route {
        Route::Connections => div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(theme.canvas)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.ink)
                            .child("No connections yet"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.ink3)
                            .child("Saved connections will appear here."),
                    ),
            ),
        Route::ConnectionForm => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.canvas)
            .child(
                div()
                    .w(px(640.0))
                    .bg(theme.panel)
                    .border_1()
                    .border_color(theme.line)
                    .rounded_lg()
                    .p_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.ink)
                            .child("New connection"),
                    )
                    .child(
                        div()
                            .mt_3()
                            .text_sm()
                            .text_color(theme.ink3)
                            .child("Connection form content arrives in a later milestone."),
                    ),
            ),
        Route::Workspace => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.canvas)
            .child(
                div()
                    .text_color(theme.ink3)
                    .child("Workspace shell (query editor + grid) lands in later milestones."),
            ),
    }
}

fn title_bar(
    cx: &mut Context<WispShell>,
    theme: &Theme,
    title: &str,
    env_label: &str,
    env_color: gpui::Rgba,
    route: Route,
) -> impl IntoElement {
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
                .gap_3()
                .bg(theme.side)
                .border_b_1()
                .border_color(theme.line)
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
                                .text_color(theme.ink2)
                                .child(format!("Wisp — {title}")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .text_color(rgb(0xFFFFFF))
                                .bg(env_color)
                                .child(env_label.to_string()),
                        ),
                )
                .child(if route == Route::Connections {
                    div()
                        .id("new-connection")
                        .text_sm()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .border_1()
                        .border_color(theme.line)
                        .bg(theme.panel)
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.side))
                        .child("New connection")
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.route = Route::ConnectionForm;
                            cx.notify();
                        }))
                } else {
                    div()
                        .id("back-connections")
                        .text_sm()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .border_1()
                        .border_color(theme.line)
                        .bg(theme.panel)
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.side))
                        .child("Back")
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.route = Route::Connections;
                            cx.notify();
                        }))
                }),
        )
        .child(div().h(px(2.0)).w_full().bg(env_color))
}

fn status_bar(
    theme: &Theme,
    env_color: gpui::Rgba,
    connection: SharedString,
    timing: SharedString,
    memory_mb: f64,
    memory_pct: f64,
) -> impl IntoElement {
    div()
        .flex_none()
        .h(px(26.0))
        .flex()
        .items_center()
        .px_3()
        .gap_4()
        .bg(theme.side)
        .border_t_1()
        .border_color(theme.line)
        .text_xs()
        .text_color(theme.ink2)
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
                                .bg(theme.line)
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
