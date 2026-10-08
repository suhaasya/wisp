//! Grid status line for the shell (row range, totals, query time).

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GridStatus {
    pub row_from: u64,
    pub row_to: u64,
    pub total_rows: Option<u64>,
    pub total_estimate: bool,
    pub query_ms: u64,
}

impl GridStatus {
    pub fn format_line(&self) -> String {
        let total = match self.total_rows {
            Some(n) if self.total_estimate => format!("~{n} rows"),
            Some(n) => format!("{n} rows"),
            None => "— rows".into(),
        };
        format!(
            "Rows {}–{} · {total} · {} ms",
            self.row_from + 1,
            self.row_to + 1,
            self.query_ms
        )
    }
}
