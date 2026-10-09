//! Query history persistence from the editor (LUM-034).

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use wisp_core::{
    append_history_entry, ConnectionId, QueryHistoryEntry, QueryHistoryStatus, ScriptStatement,
    StatementRunOutcome, WispPaths,
};

pub fn record_run_history(
    connection_id: ConnectionId,
    history_enabled: bool,
    statements: &[ScriptStatement],
    outcomes: &[StatementRunOutcome],
    cancelled: bool,
    started: Instant,
) {
    let duration_ms = started.elapsed().as_millis() as u64;
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let paths = WispPaths::resolve();
    let path = paths.query_history_jsonl(connection_id);

    if cancelled && outcomes.is_empty() {
        let _ = append_history_entry(
            &path,
            history_enabled,
            QueryHistoryEntry::new(
                statements
                    .first()
                    .map(|s| s.sql.clone())
                    .unwrap_or_default(),
                unix_ms,
                duration_ms,
                None,
                QueryHistoryStatus::Cancelled,
            ),
        );
        return;
    }

    for outcome in outcomes {
        let (stmt_index, byte_start, status, rows) = match outcome {
            StatementRunOutcome::Message {
                stmt_index,
                byte_start,
                rows_affected,
                ..
            } => (
                *stmt_index,
                *byte_start,
                QueryHistoryStatus::Ok,
                Some(*rows_affected),
            ),
            StatementRunOutcome::ResultSet {
                stmt_index,
                byte_start,
                row_count,
                ..
            } => {
                let rows = match row_count {
                    wisp_core::RowCount::Exact(n) => Some(*n),
                    _ => None,
                };
                (
                    *stmt_index,
                    *byte_start,
                    QueryHistoryStatus::Ok,
                    rows,
                )
            }
            StatementRunOutcome::Explain { stmt_index, byte_start, .. } => (
                *stmt_index,
                *byte_start,
                QueryHistoryStatus::Ok,
                None,
            ),
            StatementRunOutcome::Failed {
                stmt_index,
                byte_start,
                ..
            } => (*stmt_index, *byte_start, QueryHistoryStatus::Error, None),
        };
        let sql = statements
            .iter()
            .find(|s| s.byte_start == byte_start)
            .map(|s| s.sql.clone())
            .or_else(|| {
                statements
                    .get(stmt_index)
                    .map(|s| s.sql.clone())
            })
            .unwrap_or_default();
        let per_ms = if outcomes.len() == 1 {
            duration_ms
        } else {
            duration_ms / outcomes.len().max(1) as u64
        };
        let _ = append_history_entry(
            &path,
            history_enabled,
            QueryHistoryEntry::new(sql, unix_ms, per_ms, rows, status),
        );
    }
}
