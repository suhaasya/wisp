//! Keyset `(sort, pk)` tuple seek SQL (stable forward paging).

use wisp_drivers::Dialect;

/// Lexicographic tuple comparison: `(c1, c2, …) > (v1, v2, …)`.
///
/// Uses row-value comparison where the dialect allows (PostgreSQL / MySQL 8+).
pub fn tuple_seek_predicate(
    dialect: &dyn Dialect,
    columns: &[&str],
    placeholders: &[&str],
) -> String {
    debug_assert_eq!(columns.len(), placeholders.len());
    if columns.is_empty() {
        return "TRUE".into();
    }
    let quoted: Vec<String> = columns.iter().map(|c| dialect.quote_identifier(c)).collect();
    let cols = quoted.join(", ");
    let vals = placeholders.join(", ");
    format!("({cols}) > ({vals})")
}

#[cfg(test)]
mod tests {
    use wisp_drivers::PostgresDialect;

    use super::*;

    #[test]
    fn postgres_tuple_seek() {
        let sql = tuple_seek_predicate(
            &PostgresDialect,
            &["created_at", "id"],
            &["$1", "$2"],
        );
        assert_eq!(sql, "(\"created_at\", \"id\") > ($1, $2)");
    }
}
