use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use gpui::{point, prelude::*, px, size, App, Bounds, WindowBounds, WindowOptions};
use gpui_platform::application;

use crate::{
    launch::{LaunchConfig, LaunchOutcome, ShellMetrics, WindowPersistence},
    memory,
    shell::WispShell,
};

pub fn run(config: LaunchConfig) -> Result<LaunchOutcome> {
    memory::start_memory_sampler();

    let persistence = Rc::new(RefCell::new(config.window));
    let metrics = Rc::new(RefCell::new(ShellMetrics::default()));

    application().run({
        let persistence = persistence.clone();
        let metrics = metrics.clone();
        move |cx: &mut App| {
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
                    move |_, cx| {
                        cx.new(move |_| WispShell::new(persistence.clone(), metrics.clone()))
                    }
                },
            )
            .expect("open wisp window");

            cx.activate(true);
        }
    });

    let window = persistence.borrow().clone();
    let shell_metrics = metrics.borrow().clone();
    Ok(LaunchOutcome {
        window,
        metrics: shell_metrics,
    })
}

fn window_bounds_from_persistence(state: &WindowPersistence, cx: &App) -> WindowBounds {
    let default_bounds = Bounds::centered(None, size(px(1080.0), px(780.0)), cx);
    if state.maximized {
        let restore = state
            .geometry
            .map(|g| Bounds::new(point(px(g.x), px(g.y)), size(px(g.width), px(g.height))))
            .unwrap_or(default_bounds);
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
