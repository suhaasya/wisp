//! Wisp application entry point.

mod bench;

use clap::Parser;
use wisp_store::window::WindowState;
use wisp_ui::{LaunchConfig, WindowGeometry, WindowPersistence};

#[derive(Parser)]
#[command(name = "wisp", version, about = "Wisp database client")]
struct Args {
    /// Hidden harness entry point for CI budget scenarios (LUM-004).
    #[arg(long, hide = true)]
    bench_scenario: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(scenario) = args.bench_scenario {
        bench::run(&scenario);
    }

    #[cfg(feature = "bench-stress")]
    {
        let _ = &*bench_stress::STRESS;
    }

    let stored = WindowState::load();
    let config = LaunchConfig {
        window: WindowPersistence {
            maximized: stored.maximized,
            geometry: stored.geometry.map(|g| WindowGeometry {
                width: g.width,
                height: g.height,
                x: g.x,
                y: g.y,
            }),
        },
    };

    let outcome = wisp_ui::run(config)?;
    let saved = WindowState {
        maximized: outcome.window.maximized,
        geometry: outcome
            .window
            .geometry
            .map(|g| wisp_store::window::WindowGeometry {
                width: g.width,
                height: g.height,
                x: g.x,
                y: g.y,
            }),
    };
    saved.save()?;

    Ok(())
}

#[cfg(feature = "bench-stress")]
mod bench_stress {
    pub static STRESS: [u8; 30 * 1024 * 1024] = [0; 30 * 1024 * 1024];
}

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(wisp_ui::CRATE_MARKER, "wisp-ui");
    }
}
