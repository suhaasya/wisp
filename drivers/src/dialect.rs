//! SQL dialect differences (quoting, literals, pagination).

/// Engine-specific SQL surface syntax.
pub trait Dialect: Send + Sync {
    fn name(&self) -> &'static str;

    fn quote_identifier(&self, ident: &str) -> String;

    fn escape_string_literal(&self, value: &str) -> String;

    /// `LIMIT` / `OFFSET` pagination clause (no leading whitespace required).
    fn limit_offset(&self, limit: u32, offset: u64) -> String;

    /// Keyset pagination predicate (`WHERE (sort cols) > (?)` style); drivers extend in LUM-012+.
    fn keyset_seek(&self, order_columns: &[&str], placeholders: &[&str]) -> String;
}

/// PostgreSQL-style double-quoted identifiers and `LIMIT`/`OFFSET`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PostgresDialect;

impl Dialect for PostgresDialect {
    fn name(&self) -> &'static str {
        "postgresql"
    }

    fn quote_identifier(&self, ident: &str) -> String {
        format!("\"{}\"", ident.replace('"', "\"\""))
    }

    fn escape_string_literal(&self, value: &str) -> String {
        format!("'{}'", value.replace('\'', "''"))
    }

    fn limit_offset(&self, limit: u32, offset: u64) -> String {
        format!("LIMIT {limit} OFFSET {offset}")
    }

    fn keyset_seek(&self, order_columns: &[&str], placeholders: &[&str]) -> String {
        debug_assert_eq!(order_columns.len(), placeholders.len());
        let pairs = order_columns
            .iter()
            .zip(placeholders.iter())
            .map(|(c, p)| format!("{} > {p}", self.quote_identifier(c)))
            .collect::<Vec<_>>();
        pairs.join(" OR ")
    }
}

/// MySQL-style backtick identifiers and `LIMIT offset, count`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MysqlDialect;

impl Dialect for MysqlDialect {
    fn name(&self) -> &'static str {
        "mysql"
    }

    fn quote_identifier(&self, ident: &str) -> String {
        format!("`{}`", ident.replace('`', "``"))
    }

    fn escape_string_literal(&self, value: &str) -> String {
        format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
    }

    fn limit_offset(&self, limit: u32, offset: u64) -> String {
        format!("LIMIT {offset}, {limit}")
    }

    fn keyset_seek(&self, order_columns: &[&str], placeholders: &[&str]) -> String {
        debug_assert_eq!(order_columns.len(), placeholders.len());
        let pairs = order_columns
            .iter()
            .zip(placeholders.iter())
            .map(|(c, p)| format!("{} > {p}", self.quote_identifier(c)))
            .collect::<Vec<_>>();
        pairs.join(" OR ")
    }
}
