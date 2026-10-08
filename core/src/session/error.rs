use thiserror::Error;

use wisp_store::ConnectionId;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SessionError {
    #[error("no session for connection {0}")]
    NotFound(ConnectionId),
    #[error("session is {phase:?}")]
    NotReady { phase: super::state::SessionPhase },
    #[error("read-only connection — writes are blocked")]
    ReadOnlyWriteBlocked,
    #[error("safe mode — confirm this write before sending")]
    WriteNeedsConfirmation { sql: String },
    #[error("connection failed: {0}")]
    Connect(String),
    #[error("query failed: {0}")]
    Query(String),
    #[error("session closed")]
    Closed,
}
