use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use gpui::{point, prelude::*, px, size, App, Bounds, WindowBounds, WindowOptions};
use gpui_platform::application;

use crate::{
    components::text_input::bind_text_input_keys,
    launch::{LaunchConfig, LaunchOutcome, ShellMetrics, WindowPersistence},
    memory,
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

    application().run({
        let persistence = persistence.clone();
        let metrics = metrics.clone();
        let appearance = appearance.clone();
        let settings_inbox = settings_inbox.clone();
        let db_bridge = db_bridge.clone();
        let connections = connections.clone();
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

            let bounds = window_bounds_from_persistence(&persistence.borrow(), cx);

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
                {
                    let persistence = persistence.clone();
                    let metrics = metrics.clone();
                    let appearance = appearance.clone();
                    let settings_inbox = settings_inbox.clone();
                    let connections = connections.clone();
                    move |_, cx| {
                        #[allow(unused_variables)]
                        cx.new(|cx| {
                            #[cfg(feature = "ui-gallery")]
                            {
                                let gallery =
                                    cx.new(crate::components::gallery::ComponentGallery::new);
                                WispShell::new(
                                    persistence.clone(),
                                    metrics.clone(),
                                    appearance.clone(),
                                    settings_inbox.clone(),
                                    connections.clone(),
                                    gallery,
                                    cx,
                                )
                            }
                            #[cfg(not(feature = "ui-gallery"))]
                            {
                                WispShell::new(
                                    persistence.clone(),
                                    metrics.clone(),
                                    appearance.clone(),
                                    settings_inbox.clone(),
                                    connections.clone(),
                                    cx,
                                )
                            }
                        })
                    }
                },
            )
            .expect("open wisp window");

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
