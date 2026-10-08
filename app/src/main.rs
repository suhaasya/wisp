//! Wisp application entry point.
//!
//! GPUI window bootstrap arrives in LUM-005; this binary only proves the workspace links.

mod bench;

use clap::Parser;

#[derive(Parser)]
#[command(name = "wisp", version, about = "Wisp database client")]
struct Args {
    /// Hidden harness entry point for CI budget scenarios (LUM-004).
    #[arg(long, hide = true)]
    bench_scenario: Option<String>,
}

fn main() {
    let args = Args::parse();
    if let Some(scenario) = args.bench_scenario {
        bench::run(&scenario);
    }

    #[cfg(feature = "bench-stress")]
    {
        // Forces the size gate to fail when built with `--features bench-stress`.
        let _ = &*bench_stress::STRESS;
    }

    println!(
        "wisp {} (ui={}, store={}) — hello window placeholder",
        env!("CARGO_PKG_VERSION"),
        wisp_ui::CRATE_MARKER,
        wisp_store::CRATE_MARKER,
    );
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
