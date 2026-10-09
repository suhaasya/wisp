//! Multi-statement script execution types (LUM-033).

use wisp_drivers::ColumnMeta;

#[derive(Debug, Clone)]
pub struct ScriptStatement {
    pub byte_start: usize,
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementRunOutcome {
    Message {
        stmt_index: usize,
        byte_start: usize,
        rows_affected: u64,
        text: String,
    },
    ResultSet {
        stmt_index: usize,
        byte_start: usize,
        sql: String,
        columns: Vec<ColumnMeta>,
        row_count: crate::pager::RowCount,
    },
    Explain {
        stmt_index: usize,
        byte_start: usize,
        plan_text: String,
    },
    Failed {
        stmt_index: usize,
        byte_start: usize,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunScriptReport {
    pub outcomes: Vec<StatementRunOutcome>,
    pub cancelled: bool,
}
