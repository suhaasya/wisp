//! Client-side write gates for read-only and safe-mode sessions.

use super::error::SessionError;

/// Heuristic write detector (complements server-side read-only session settings).
pub fn sql_looks_like_write(sql: &str) -> bool {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return false;
    }
    let head = trimmed
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_start_matches('(')
        .to_ascii_uppercase();
    matches!(
        head.as_str(),
        "INSERT"
            | "UPDATE"
            | "DELETE"
            | "MERGE"
            | "REPLACE"
            | "TRUNCATE"
            | "DROP"
            | "ALTER"
            | "CREATE"
            | "GRANT"
            | "REVOKE"
            | "CALL"
            | "EXEC"
            | "EXECUTE"
    ) || trimmed.to_ascii_uppercase().starts_with("WITH")
        && trimmed.to_ascii_uppercase().contains(" INSERT ")
}

pub fn check_write_allowed(
    read_only: bool,
    safe_mode: bool,
    write_approved: bool,
    sql: &str,
) -> Result<(), SessionError> {
    if !sql_looks_like_write(sql) {
        return Ok(());
    }
    if read_only {
        return Err(SessionError::ReadOnlyWriteBlocked);
    }
    if safe_mode && !write_approved {
        return Err(SessionError::WriteNeedsConfirmation {
            sql: sql.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_writes() {
        assert!(sql_looks_like_write("INSERT INTO t VALUES (1)"));
        assert!(sql_looks_like_write("  update foo set x=1"));
        assert!(!sql_looks_like_write("SELECT 1"));
    }

    #[test]
    fn read_only_blocks_writes() {
        assert_eq!(
            check_write_allowed(true, false, false, "DELETE FROM t"),
            Err(SessionError::ReadOnlyWriteBlocked)
        );
        assert!(check_write_allowed(true, false, false, "SELECT 1").is_ok());
    }

    #[test]
    fn safe_mode_requires_approval() {
        assert!(matches!(
            check_write_allowed(false, true, false, "INSERT INTO t VALUES (1)"),
            Err(SessionError::WriteNeedsConfirmation { .. })
        ));
        assert!(check_write_allowed(false, true, true, "INSERT INTO t VALUES (1)").is_ok());
    }
}
