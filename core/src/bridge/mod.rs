//! Tokio runtime isolated from GPUI; commands in, events out (LUM-010).

mod payload;
mod runtime;
mod thread;

pub use payload::{DbCommandPayload, DbEvent, DbEventPayload};
pub use runtime::{DbBridge, DbRuntimeConfig};
pub use thread::{assert_no_block_on_ui, enter_runtime_thread, enter_ui_thread};

use std::sync::atomic::{AtomicU64, Ordering};

/// Monotonic id correlating a command with its completion event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(u64);

impl RequestId {
    pub fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        thread,
        time::{Duration, Instant},
    };

    use super::*;
    use crate::error::WispErrorKind;

    #[test]
    fn thousand_concurrent_requests_complete() {
        let bridge = Arc::new(DbBridge::start(DbRuntimeConfig::default()));
        let mut threads = Vec::new();
        for t in 0..10u64 {
            let bridge = Arc::clone(&bridge);
            threads.push(thread::spawn(move || {
                for i in 0..100u64 {
                    let seq = t * 100 + i;
                    let (_, _, rx) = bridge.submit(DbCommandPayload::Seq(seq));
                    let event = rx.recv().expect("event");
                    assert_eq!(event.result().expect("ok"), DbEventPayload::Seq(seq));
                }
            }));
        }
        for handle in threads {
            handle.join().expect("thread");
        }
    }

    #[test]
    fn cancel_stops_sleep_within_100ms() {
        let bridge = DbBridge::start(DbRuntimeConfig::default());
        let (_id, cancel, rx) = bridge.submit(DbCommandPayload::SleepMs(30_000));
        let start = Instant::now();
        cancel.cancel();
        let event = rx.recv_timeout(Duration::from_millis(100)).expect("event");
        assert!(start.elapsed() < Duration::from_millis(100));
        assert_eq!(event.result().unwrap_err().kind(), WispErrorKind::Cancelled);
    }

    #[test]
    fn ten_second_job_does_not_block_other_work() {
        let bridge = DbBridge::start(DbRuntimeConfig::default());
        let (_long_id, _long_cancel, _long_rx) =
            bridge.submit(DbCommandPayload::SleepMs(10_000));
        let start = Instant::now();
        for i in 0..50u64 {
            let (_, _, rx) = bridge.submit(DbCommandPayload::Seq(i));
            let event = rx.recv_timeout(Duration::from_secs(1)).expect("event");
            assert_eq!(event.result().expect("ok"), DbEventPayload::Seq(i));
        }
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn submit_returns_immediately() {
        let bridge = DbBridge::start(DbRuntimeConfig::default());
        let start = Instant::now();
        let (_id, _cancel, rx) = bridge.submit(DbCommandPayload::SleepMs(500));
        assert!(start.elapsed() < Duration::from_millis(5));
        let _ = rx.recv_timeout(Duration::from_secs(2)).expect("done");
    }
}
