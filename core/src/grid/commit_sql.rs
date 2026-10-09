//! Generate DML for staged grid edits (LUM-029).

use std::collections::HashMap;

use wisp_drivers::{ColumnMeta, Dialect};

use crate::query::{sql_for_display, BuiltSql, SqlParam};

use super::change_set::{CellEdit, InsertedRow, PrimaryKey, RowKey, TableChangeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitStatementKind {
    Insert,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitStatement {
    pub kind: CommitStatementKind,
    pub row_key: RowKey,
    pub built: BuiltSql,
}

#[derive(Debug, Clone)]
pub struct GridCommitSpec {
    pub schema: String,
    pub table: String,
    pub columns: Vec<ColumnMeta>,
    pub optimistic: bool,
}

pub fn build_commit_statements(
    dialect: &dyn Dialect,
    spec: &GridCommitSpec,
    change_set: &TableChangeSet,
    baselines: &HashMap<PrimaryKey, HashMap<String, String>>,
) -> Vec<CommitStatement> {
    let mut out = Vec::new();
    let table_ref = format!(
        "{}.{}",
        dialect.quote_identifier(&spec.schema),
        dialect.quote_identifier(&spec.table)
    );

    for ins in change_set.inserts() {
        out.push(build_insert(dialect, &table_ref, ins));
    }

    for (key, edits) in change_set.updated_rows() {
        let RowKey::Pk(pk) = key else {
            continue;
        };
        if let Some(stmt) = build_update(
            dialect,
            &table_ref,
            pk,
            edits,
            change_set,
            baselines,
            spec.optimistic,
        ) {
            out.push(stmt);
        }
    }

    for key in change_set.deleted_pk_keys() {
        let RowKey::Pk(pk) = key else {
            continue;
        };
        out.push(build_delete(dialect, &table_ref, pk, change_set));
    }

    out
}

/// Rendered SQL strings for review and execution (same generator).
pub fn sql_for_execution(statements: &[CommitStatement], dialect: &dyn Dialect) -> Vec<String> {
    statements
        .iter()
        .map(|s| sql_for_display(&s.built, dialect))
        .collect()
}

fn build_insert(dialect: &dyn Dialect, table_ref: &str, row: &InsertedRow) -> CommitStatement {
    let mut cols: Vec<_> = row.cells.keys().cloned().collect();
    cols.sort();
    let mut params = Vec::new();
    let col_sql: Vec<_> = cols
        .iter()
        .map(|c| dialect.quote_identifier(c))
        .collect();
    let placeholders: Vec<_> = cols
        .iter()
        .map(|c| {
            push_param(&mut params, &CellEdit::Set(row.cells[c].clone()));
            placeholder(dialect, params.len())
        })
        .collect();
    let sql = format!(
        "INSERT INTO {table_ref} ({}) VALUES ({})",
        col_sql.join(", "),
        placeholders.join(", ")
    );
    CommitStatement {
        kind: CommitStatementKind::Insert,
        row_key: RowKey::Insert(row.temp_id),
        built: BuiltSql { sql, params },
    }
}

fn build_update(
    dialect: &dyn Dialect,
    table_ref: &str,
    pk: &PrimaryKey,
    edits: &HashMap<String, CellEdit>,
    change_set: &TableChangeSet,
    baselines: &HashMap<PrimaryKey, HashMap<String, String>>,
    optimistic: bool,
) -> Option<CommitStatement> {
    if edits.is_empty() {
        return None;
    }
    let mut params = Vec::new();
    let mut sets: Vec<String> = Vec::new();
    let mut keys: Vec<_> = edits.keys().cloned().collect();
    keys.sort();
    for col in keys {
        let edit = edits.get(&col)?;
        if change_set.pk_columns.iter().any(|p| p == &col) {
            continue;
        }
        let col_q = dialect.quote_identifier(&col);
        let rhs = match edit {
            CellEdit::Default => "DEFAULT".into(),
            CellEdit::Null | CellEdit::Set(_) => {
                push_param(&mut params, edit);
                placeholder(dialect, params.len())
            }
        };
        sets.push(format!("{col_q} = {rhs}"));
    }
    if sets.is_empty() {
        return None;
    }
    let (where_sql, _) = pk_where(dialect, pk, change_set, &mut params);
    let mut where_clause = where_sql;
    if optimistic {
        if let Some(base_row) = baselines.get(pk) {
            for (col, edit) in edits {
                if change_set.pk_columns.iter().any(|p| p == col) {
                    continue;
                }
                if let Some(orig) = base_row.get(col) {
                    let col_q = dialect.quote_identifier(col);
                    params.push(SqlParam::String(orig.clone()));
                    where_clause.push_str(&format!(
                        " AND {col_q} = {}",
                        placeholder(dialect, params.len())
                    ));
                }
            }
        }
    }
    let sql = format!(
        "UPDATE {table_ref} SET {} WHERE {where_clause}",
        sets.join(", ")
    );
    Some(CommitStatement {
        kind: CommitStatementKind::Update,
        row_key: RowKey::Pk(pk.clone()),
        built: BuiltSql { sql, params },
    })
}

fn build_delete(
    dialect: &dyn Dialect,
    table_ref: &str,
    pk: &PrimaryKey,
    change_set: &TableChangeSet,
) -> CommitStatement {
    let mut params = Vec::new();
    let (where_clause, _) = pk_where(dialect, pk, change_set, &mut params);
    let sql = format!("DELETE FROM {table_ref} WHERE {where_clause}");
    CommitStatement {
        kind: CommitStatementKind::Delete,
        row_key: RowKey::Pk(pk.clone()),
        built: BuiltSql { sql, params },
    }
}

fn pk_where(
    dialect: &dyn Dialect,
    pk: &PrimaryKey,
    change_set: &TableChangeSet,
    params: &mut Vec<SqlParam>,
) -> (String, RowKey) {
    let mut parts = Vec::new();
    for pk_col in &change_set.pk_columns {
        let value = pk
            .0
            .iter()
            .find(|(c, _)| c == pk_col)
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        params.push(SqlParam::String(value.to_string()));
        parts.push(format!(
            "{} = {}",
            dialect.quote_identifier(pk_col),
            placeholder(dialect, params.len())
        ));
    }
    (parts.join(" AND "), RowKey::Pk(pk.clone()))
}

fn push_param(params: &mut Vec<SqlParam>, edit: &CellEdit) {
    match edit {
        CellEdit::Null => params.push(SqlParam::Null),
        CellEdit::Default => params.push(SqlParam::Null),
        CellEdit::Set(s) => {
            if let Ok(n) = s.parse::<i64>() {
                params.push(SqlParam::Integer(n));
            } else {
                params.push(SqlParam::String(s.clone()));
            }
        }
    }
}

fn placeholder(dialect: &dyn Dialect, param_index: usize) -> String {
    if dialect.name() == "mysql" {
        "?".into()
    } else {
        format!("${param_index}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::sql_for_display;
    use wisp_drivers::PostgresDialect;

    #[test]
    fn update_uses_pk_in_where() {
        let mut cs = TableChangeSet::with_pk_columns(vec!["id".into()]);
        let pk = PrimaryKey::single("id", "7");
        cs.stage_cell(
            RowKey::Pk(pk.clone()),
            "name",
            CellEdit::Set("n".into()),
        );
        let spec = GridCommitSpec {
            schema: "public".into(),
            table: "orders".into(),
            columns: vec![],
            optimistic: false,
        };
        let stmts = build_commit_statements(&PostgresDialect, &spec, &cs, &HashMap::new());
        assert_eq!(stmts.len(), 1);
        let display = sql_for_display(&stmts[0].built, &PostgresDialect);
        assert!(display.contains("UPDATE"));
        assert!(display.contains("WHERE"));
        assert!(display.contains("id = 7") || display.contains("id = '7'"));
    }

    #[test]
    fn display_matches_execution_strings() {
        let mut cs = TableChangeSet::with_pk_columns(vec!["id".into()]);
        cs.delete_row(RowKey::Pk(PrimaryKey::single("id", "1")));
        let spec = GridCommitSpec {
            schema: "public".into(),
            table: "t".into(),
            columns: vec![],
            optimistic: false,
        };
        let stmts = build_commit_statements(&PostgresDialect, &spec, &cs, &HashMap::new());
        let exec = sql_for_execution(&stmts, &PostgresDialect);
        for (stmt, line) in stmts.iter().zip(exec.iter()) {
            assert_eq!(line, &sql_for_display(&stmt.built, &PostgresDialect));
        }
    }
}
