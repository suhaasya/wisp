//! Command and event payloads for the DB bridge.

use super::RequestId;
use crate::{
    session::{SessionError, SessionOpenSpec, SessionSnapshot},
    ConnectionTestError, ConnectionTestOutcome, ConnectionTestSpec, WispError,
};
use wisp_drivers::ExecuteStats;
use wisp_store::ConnectionId;

/// Work dispatched to the tokio runtime.
#[derive(Debug, Clone)]
pub enum DbCommandPayload {
    /// Test / harness: return the same sequence number in the event.
    Seq(u64),
    /// Test / harness: sleep up to `ms` unless cancelled.
    SleepMs(u64),
    /// Connection form: connect + ping + server info (LUM-016).
    TestConnection(ConnectionTestSpec),
    /// Open a long-lived session (LUM-020).
    SessionOpen(SessionOpenSpec),
    /// Close one session.
    SessionClose(ConnectionId),
    /// Run SQL on an open session (respects read-only / safe-mode).
    SessionExecute {
        id: ConnectionId,
        sql: String,
        write_approved: bool,
    },
    /// Query session state for the status bar.
    SessionSnapshot(ConnectionId),
    /// Close every live session (app quit).
    ShutdownAllSessions,
}

/// Successful completion data.
#[derive(Debug, Clone, PartialEq)]
pub enum DbEventPayload {
    Seq(u64),
    Unit,
    ConnectionTest(Result<ConnectionTestOutcome, ConnectionTestError>),
    SessionOpen(Result<SessionSnapshot, SessionError>),
    SessionClose,
    SessionExecute(Result<ExecuteStats, SessionError>),
    SessionSnapshot(Option<SessionSnapshot>),
    ShutdownAllSessions,
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
