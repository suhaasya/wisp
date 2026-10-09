//! Command and event payloads for the DB bridge.

use super::RequestId;
use crate::{
    schema::SchemaLoadResult,
    session::{
        CommitTransactionOutcome, QueryPageSnapshot, RunScriptReport, ScriptStatement,
        SessionError, SessionOpenSpec, SessionSnapshot,
    },
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
    /// Staged grid commit (LUM-029): one transaction, full rollback on failure.
    SessionCommitTransaction {
        id: ConnectionId,
        statements: Vec<String>,
        write_approved: bool,
    },
    /// Query session state for the status bar.
    SessionSnapshot(ConnectionId),
    /// Run a script (multi-statement) on an open session (LUM-033).
    SessionRunScript {
        id: ConnectionId,
        statements: Vec<ScriptStatement>,
        write_approved: bool,
    },
    /// Fetch one page of a prior SELECT (LUM-033).
    SessionQueryPage {
        id: ConnectionId,
        sql: String,
        offset: u64,
        limit: u32,
    },
    /// Cancel the in-flight query on a session (LUM-033).
    SessionCancelQuery(ConnectionId),
    /// Load schema catalog via metadata connection (LUM-021).
    SessionFetchSchemaCatalog(ConnectionId),
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
    SessionCommitTransaction(Result<CommitTransactionOutcome, SessionError>),
    SessionSnapshot(Option<SessionSnapshot>),
    SessionRunScript(Result<RunScriptReport, SessionError>),
    SessionQueryPage(Result<QueryPageSnapshot, SessionError>),
    SessionCancelQuery,
    SessionSchemaCatalog(Result<SchemaLoadResult, SessionError>),
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
