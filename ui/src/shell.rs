use std::{cell::RefCell, rc::Rc, time::Instant};

use gpui::{
    div, prelude::*, px, Context, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, WindowBounds,
};

#[cfg(feature = "ui-gallery")]
use gpui::Entity;

#[cfg(feature = "ui-gallery")]
use crate::components::gallery::ComponentGallery;


use crate::{
    environment::Environment,
    launch::{AppearanceConfig, ShellMetrics, WindowPersistence},
    memory,
    route::Route,
    theme::{self, ResolvedTheme},
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
    #[cfg(feature = "ui-gallery")]
    gallery: Entity<ComponentGallery>,
}

impl WispShell {
    pub fn new(
        persistence: Rc<RefCell<WindowPersistence>>,
        metrics: Rc<RefCell<ShellMetrics>>,
        appearance: Rc<RefCell<AppearanceConfig>>,
        #[cfg(feature = "ui-gallery")] gallery: Entity<ComponentGallery>,
    ) -> Self {
        Self {
            route: Route::Connections,
            environment: Environment::Local,
            connection_status: "Not connected".into(),
            timing_status: "Ready".into(),
            launch_started: Instant::now(),
            first_frame_reported: false,
            persistence,
            metrics,
            appearance,
            #[cfg(feature = "ui-gallery")]
            gallery,
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

        let theme = theme::read_global(cx).resolved().clone();
        let colors = &theme.colors;
        let env_color = self.environment.color(&colors.env);
        let env_on_color = self.environment.on_color(&colors.env);
        let route = self.route;
        let title = route.title();
        let env_label = self.environment.label();
        let memory_mb = memory::memory_megabytes();
        let memory_pct = (memory_mb / 40.0).clamp(0.0, 1.0);
        let connection = self.connection_status.clone();
        let timing = self.timing_status.clone();
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
    }
}

#[allow(unused_variables)]
fn render_route(shell: &WispShell, route: Route, theme: &ResolvedTheme) -> impl IntoElement {
    let c = &theme.colors;
    match route {
        #[cfg(feature = "ui-gallery")]
        Route::Gallery => div().size_full().child(shell.gallery.clone()),
        Route::Connections => div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(c.canvas)
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
                            .text_color(c.ink1)
                            .child("No connections yet"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(c.ink3)
                            .child("Saved connections will appear here."),
                    ),
            ),
        Route::ConnectionForm => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(c.canvas)
            .child(
                div()
                    .w(px(640.0))
                    .bg(c.panel)
                    .border_1()
                    .border_color(c.line)
                    .rounded_lg()
                    .p_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(c.ink1)
                            .child("New connection"),
                    )
                    .child(
                        div()
                            .mt_3()
                            .text_sm()
                            .text_color(c.ink3)
                            .child("Connection form content arrives in a later milestone."),
                    ),
            ),
        Route::Workspace => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(c.canvas)
            .child(
                div()
                    .text_color(c.ink3)
                    .child("Workspace shell (query editor + grid) lands in later milestones."),
            ),
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
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .text_color(env_on_color)
                                .bg(env_color)
                                .child(env_label.to_string()),
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
        .child(div().h(px(2.0)).w_full().bg(env_color))
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
