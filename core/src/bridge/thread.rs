//! UI vs runtime thread markers (debug guard against `block_on` on GPUI thread).

use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThreadRole {
    Unknown,
    Ui,
    Runtime,
}

thread_local! {
    static ROLE: Cell<ThreadRole> = const { Cell::new(ThreadRole::Unknown) };
}

/// Mark the calling thread as the GPUI main thread (call once at app startup).
pub fn enter_ui_thread() {
    ROLE.with(|r| r.set(ThreadRole::Ui));
}

/// Mark the calling thread as the dedicated tokio driver thread.
pub fn enter_runtime_thread() {
    ROLE.with(|r| r.set(ThreadRole::Runtime));
}

/// Panics in debug builds if called from the UI thread.
pub fn assert_no_block_on_ui() {
    debug_assert!(
        ROLE.with(|r| r.get()) != ThreadRole::Ui,
        "must not block_on or block the GPUI UI thread"
    );
}
