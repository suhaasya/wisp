//! Command and event payloads for the DB bridge (simulated work until LUM-012).

use super::RequestId;
use crate::error::WispError;

/// Work dispatched to the tokio runtime (drivers will extend this enum later).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbCommandPayload {
    /// Test / harness: return the same sequence number in the event.
    Seq(u64),
    /// Test / harness: sleep up to `ms` unless cancelled.
    SleepMs(u64),
}

/// Successful completion data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbEventPayload {
    Seq(u64),
    Unit,
}

/// Completion event for a single request.
#[derive(Debug, Clone)]
pub struct DbEvent {
    pub id: RequestId,
    result: Result<DbEventPayload, WispError>,
}

impl DbEvent {
    pub fn completed(id: RequestId, result: Result<DbEventPayload, WispError>) -> Self {
        Self { id, result }
    }

    pub fn result(&self) -> Result<DbEventPayload, WispError> {
        self.result.clone()
    }
}
