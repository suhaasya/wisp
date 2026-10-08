//! Process RSS sampler (background thread, not on the render path).

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
    thread,
    time::Duration,
};

use sysinfo::{ProcessesToUpdate, System};

static RSS_BYTES: AtomicU64 = AtomicU64::new(0);
static SAMPLER: OnceLock<()> = OnceLock::new();

pub fn start_memory_sampler() {
    SAMPLER.get_or_init(|| {
        thread::spawn(|| {
            let pid = sysinfo::get_current_pid().expect("current pid");
            let mut system = System::new();
            loop {
                system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
                if let Some(process) = system.process(pid) {
                    RSS_BYTES.store(process.memory(), Ordering::Relaxed);
                }
                thread::sleep(Duration::from_secs(2));
            }
        });
    });
}

pub fn memory_bytes() -> u64 {
    RSS_BYTES.load(Ordering::Relaxed)
}

pub fn memory_megabytes() -> f64 {
    memory_bytes() as f64 / (1024.0 * 1024.0)
}
