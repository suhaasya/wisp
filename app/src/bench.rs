//! Headless benchmark scenarios driven by `cargo xtask budgets gate`.

use std::{thread, time::Duration};

/// Run a named scenario until the parent harness terminates the process.
pub fn run(scenario: &str) -> ! {
    match scenario {
        "idle" => bench_idle(),
        "grid_scroll_1m" => bench_grid_scroll_1m(),
        "tabs_20" => bench_tabs_20(),
        other => {
            eprintln!("unknown bench scenario: {other}");
            std::process::exit(2);
        }
    }
}

fn bench_idle() -> ! {
    // GC-equivalent: drop transient allocations before the settle window.
    shrink_heaps();
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}

fn bench_tabs_20() -> ! {
    let result = wisp_core::bench_tabs_20();
    eprintln!(
        "wisp-tabs-20: tabs={} hot_bytes={} background_bytes={}",
        result.tab_count, result.hot_loaded_bytes, result.background_loaded_bytes
    );
    shrink_heaps();
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}

fn bench_grid_scroll_1m() -> ! {
    let result = wisp_core::bench_scroll_1m();
    eprintln!(
        "wisp-grid-scroll-1m: rows={} steps={} p99_us={} total_ms={}",
        result.row_count, result.viewport_steps, result.p99_step_us, result.total_ms
    );
    std::process::exit(0);
}

fn shrink_heaps() {
    let mut v = Vec::<u8>::with_capacity(1024 * 1024);
    v.clear();
    drop(v);
}
