//! Headless benchmark scenarios driven by `cargo xtask budgets gate`.

use std::{thread, time::Duration};

/// Run a named scenario until the parent harness terminates the process.
pub fn run(scenario: &str) -> ! {
    match scenario {
        "idle" => bench_idle(),
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

fn shrink_heaps() {
    let mut v = Vec::<u8>::with_capacity(1024 * 1024);
    v.clear();
    drop(v);
}
