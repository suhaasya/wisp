//! Statement classification for query execution (LUM-033).

/// True when the statement likely returns a row set (SELECT, SHOW, EXPLAIN, etc.).
pub fn sql_returns_rows(sql: &str) -> bool {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return false;
    }
    let upper = trimmed.to_ascii_uppercase();
    if upper.starts_with("EXPLAIN") {
        return true;
    }
    if upper.starts_with("SELECT") || upper.starts_with("WITH") || upper.starts_with("SHOW") {
        return true;
    }
    if upper.starts_with("TABLE ") {
        return true;
    }
    false
}

pub fn sql_is_explain(sql: &str) -> bool {
    sql.trim()
        .split_whitespace()
        .next()
        .is_some_and(|w| w.eq_ignore_ascii_case("EXPLAIN"))
}

pub fn sql_is_explain_analyze(sql: &str) -> bool {
    let upper = sql.trim().to_ascii_uppercase();
    upper.starts_with("EXPLAIN") && upper.contains(" ANALYZE")
}
