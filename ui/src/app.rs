use std::{cell::RefCell, rc::Rc, sync::Arc};

use anyhow::Result;
use gpui::{point, prelude::*, px, size, App, Bounds, Global, WindowBounds, WindowOptions};
use gpui_platform::application;
use wisp_core::ConnectionId;

use crate::{
    components::text_input::bind_text_input_keys,
    launch::{AppearanceConfig, LaunchConfig, LaunchOutcome, ShellMetrics, WindowGeometry, WindowPersistence},
    memory,
    multi_window::{ShellBoot, WindowOpenQueue, WindowSpawner},
    shell::WispShell,
    theme,
};

pub fn run(config: LaunchConfig) -> Result<LaunchOutcome> {
    memory::start_memory_sampler();

    let persistence = Rc::new(RefCell::new(config.window));
    let metrics = Rc::new(RefCell::new(ShellMetrics::default()));
    let appearance = Rc::new(RefCell::new(config.appearance));
    let settings_inbox = config.settings_inbox.clone();
    let db_bridge = config.db_bridge.clone();
    let connections = config.connections.clone();
    let workspace_sessions = Arc::clone(&config.workspace_sessions);
    let window_open_queue = config.window_open_queue.clone();
    let initial_connection = config.initial_connection;

    let boot = Rc::new(ShellBoot {
        persistence: persistence.clone(),
        metrics: metrics.clone(),
        appearance: appearance.clone(),
        settings_inbox: settings_inbox.clone(),
        connections: connections.clone(),
        workspace_sessions: workspace_sessions.clone(),
        window_open_queue: window_open_queue.clone(),
    });

    application().run({
        let persistence = persistence.clone();
        let appearance = appearance.clone();
        let boot = boot.clone();
        let window_open_queue = window_open_queue.clone();
        move |cx: &mut App| {
            if let Some(bridge) = db_bridge.clone() {
                crate::bridge::init_db_bridge(cx, bridge);
            }

            let prefs = appearance.borrow();
            theme::init_global(
                cx,
                prefs.theme_mode,
                prefs.density,
                prefs.ui_font,
                prefs.mono_font,
            );
            drop(prefs);
            bind_text_input_keys(cx);
            crate::connections::bind_connection_keys(cx);
            crate::grid::init_grid_status(cx);

            cx.set_global(WindowSpawner {
                boot: boot.clone(),
                queue: window_open_queue.clone(),
            });

            let bounds = window_bounds_from_persistence(&persistence.borrow(), cx);
            open_shell_window(cx, &boot, bounds, initial_connection);
            cx.activate(true);
        }
    });

    let window = persistence.borrow().clone();
    let shell_metrics = metrics.borrow().clone();
    let appearance = appearance.borrow().clone();
    Ok(LaunchOutcome {
        window,
        appearance,
        metrics: shell_metrics,
    })
}

pub(crate) fn open_shell_window(
    cx: &mut App,
    boot: &Rc<ShellBoot>,
    bounds: WindowBounds,
    initial_connection: Option<ConnectionId>,
) {
    let boot = boot.clone();
    cx.open_window(
        WindowOptions {
            window_bounds: Some(bounds),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("Wisp".into()),
                appears_transparent: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        move |_, cx| {
            cx.new(|cx| {
                #[cfg(feature = "ui-gallery")]
                {
                    let gallery = cx.new(crate::components::gallery::ComponentGallery::new);
                    WispShell::new(
                        boot.persistence.clone(),
                        boot.metrics.clone(),
                        boot.appearance.clone(),
                        boot.settings_inbox.clone(),
                        boot.connections.clone(),
                        boot.workspace_sessions.clone(),
                        boot.window_open_queue.clone(),
                        initial_connection,
                        gallery,
                        cx,
                    )
                }
                #[cfg(not(feature = "ui-gallery"))]
                {
                    WispShell::new(
                        boot.persistence.clone(),
                        boot.metrics.clone(),
                        boot.appearance.clone(),
                        boot.settings_inbox.clone(),
                        boot.connections.clone(),
                        boot.workspace_sessions.clone(),
                        boot.window_open_queue.clone(),
                        initial_connection,
                        cx,
                    )
                }
            })
        },
    )
    .expect("open wisp window");
}

pub(crate) fn offset_window_bounds(
    state: &WindowPersistence,
    offset: f32,
    cx: &App,
) -> WindowBounds {
    let mut geometry = state.geometry.clone().unwrap_or(WindowGeometry {
        width: 1080.0,
        height: 780.0,
        x: 0.0,
        y: 0.0,
    });
    geometry.x += offset;
    geometry.y += offset;
    WindowBounds::Windowed(Bounds::new(
        point(px(geometry.x), px(geometry.y)),
        size(px(geometry.width), px(geometry.height)),
    ))
}

fn window_bounds_from_persistence(state: &WindowPersistence, cx: &App) -> WindowBounds {
    let default_bounds = Bounds::centered(None, size(px(1080.0), px(780.0)), cx);
    if state.maximized {
        let restore = state.geometry.map_or(default_bounds, |g| {
            Bounds::new(point(px(g.x), px(g.y)), size(px(g.width), px(g.height)))
        });
        return WindowBounds::Maximized(restore);
    }
    if let Some(g) = &state.geometry {
        return WindowBounds::Windowed(Bounds::new(
            point(px(g.x), px(g.y)),
            size(px(g.width), px(g.height)),
        ));
    }
    WindowBounds::Windowed(default_bounds)
}
