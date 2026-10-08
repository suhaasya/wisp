use wisp_store::ConnectionId;

/// Lifecycle state for a managed database session (LUM-020).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    Connecting,
    Ready,
    Busy,
    Reconnecting,
    Failed,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub connection_id: ConnectionId,
    pub name: String,
    pub phase: SessionPhase,
    pub read_only: bool,
    pub safe_mode: bool,
    pub version: Option<String>,
    pub detail: Option<String>,
}

impl SessionSnapshot {
    pub fn status_label(&self) -> &'static str {
        match self.phase {
            SessionPhase::Connecting => "Connecting…",
            SessionPhase::Ready => "Connected",
            SessionPhase::Busy => "Busy",
            SessionPhase::Reconnecting => "Reconnecting…",
            SessionPhase::Failed => "Connection failed",
            SessionPhase::Closed => "Closed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub connection_id: ConnectionId,
    pub phase: SessionPhase,
    pub detail: Option<String>,
}
