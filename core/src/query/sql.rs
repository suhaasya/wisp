//! Parameterized SELECT generation (no user value string concatenation).

use wisp_drivers::Dialect;

use super::filter::{FilterCombine, FilterModel, FilterOperator, FilterTerm};
use super::sort::{SortDirection, SortModel};
use super::spec::TableDataQuery;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlParam {
    Null,
    String(String),
    Integer(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltSql {
    /// SQL with `$1` / `?` placeholders for display and execution.
    pub sql: String,
    pub params: Vec<SqlParam>,
}

pub fn build_table_select(
    dialect: &dyn Dialect,
    query: &TableDataQuery,
    limit: u32,
    offset: u64,
) -> BuiltSql {
    let mut params = Vec::new();
    let table_ref = format!(
        "{}.{}",
        dialect.quote_identifier(&query.schema),
        dialect.quote_identifier(&query.table)
    );
    let where_clause = build_where(dialect, &query.filter, &mut params);
    let order = build_order_by(dialect, &query.sort);
    let mut sql = format!("SELECT * FROM {table_ref}");
    if let Some(w) = where_clause {
        sql.push_str(" WHERE ");
        sql.push_str(&w);
    }
    if !order.is_empty() {
        sql.push_str(" ORDER BY ");
        sql.push_str(&order);
    }
    sql.push(' ');
    sql.push_str(&dialect.limit_offset(limit, offset));
    BuiltSql { sql, params }
}

pub fn sql_for_display(built: &BuiltSql, dialect: &dyn Dialect) -> String {
    let mut out = built.sql.clone();
    for (i, param) in built.params.iter().enumerate() {
        let placeholder = placeholder_label(dialect, i);
        let literal = param_display(param);
        out = out.replace(&placeholder, &literal);
    }
    out
}

fn placeholder_label(dialect: &dyn Dialect, index: usize) -> String {
    if dialect.name() == "mysql" {
        "?".into()
    } else {
        format!("${}", index + 1)
    }
}

fn param_display(param: &SqlParam) -> String {
    match param {
        SqlParam::Null => "NULL".into(),
        SqlParam::Integer(n) => n.to_string(),
        SqlParam::String(s) => format!("'{}'", s.replace('\'', "''")),
    }
}

fn build_where(
    dialect: &dyn Dialect,
    filter: &FilterModel,
    params: &mut Vec<SqlParam>,
) -> Option<String> {
    if let Some(raw) = filter.raw_where.as_ref().filter(|s| !s.trim().is_empty()) {
        return Some(format!("({raw})"));
    }
    let parts: Vec<String> = filter
        .terms
        .iter()
        .filter_map(|t| compile_term(dialect, t, params))
        .collect();
    if parts.is_empty() {
        return None;
    }
    let join = match filter.combine {
        FilterCombine::And => " AND ",
        FilterCombine::Or => " OR ",
    };
    Some(parts.join(join))
}

fn compile_term(
    dialect: &dyn Dialect,
    term: &FilterTerm,
    params: &mut Vec<SqlParam>,
) -> Option<String> {
    if term.column.is_empty() {
        return None;
    }
    let col = dialect.quote_identifier(&term.column);
    match term.operator {
        FilterOperator::IsNull => Some(format!("{col} IS NULL")),
        FilterOperator::IsNotNull => Some(format!("{col} IS NOT NULL")),
        FilterOperator::In => {
            let values = parse_in_list(&term.value);
            if values.is_empty() {
                return None;
            }
            let placeholders: Vec<String> = values
                .iter()
                .map(|v| {
                    push_string_param(params, v);
                    placeholder_at(dialect, params.len())
                })
                .collect();
            Some(format!("{col} IN ({})", placeholders.join(", ")))
        }
        FilterOperator::Between => {
            if term.value.is_empty() || term.value_to.is_empty() {
                return None;
            }
            push_string_param(params, &term.value);
            let a = placeholder_at(dialect, params.len());
            push_string_param(params, &term.value_to);
            let b = placeholder_at(dialect, params.len());
            Some(format!("{col} BETWEEN {a} AND {b}"))
        }
        FilterOperator::Like | FilterOperator::ILike => {
            if term.value.is_empty() {
                return None;
            }
            push_string_param(params, &term.value);
            let p = placeholder_at(dialect, params.len());
            if term.operator == FilterOperator::ILike && dialect.name() == "postgresql" {
                Some(format!("{col} ILIKE {p}"))
            } else {
                Some(format!("{col} LIKE {p}"))
            }
        }
        _ => {
            if term.value.is_empty() {
                return None;
            }
            push_string_param(params, &term.value);
            let p = placeholder_at(dialect, params.len());
            let op = match term.operator {
                FilterOperator::Eq => "=",
                FilterOperator::Ne => "!=",
                FilterOperator::Lt => "<",
                FilterOperator::Gt => ">",
                FilterOperator::Lte => "<=",
                FilterOperator::Gte => ">=",
                _ => "=",
            };
            Some(format!("{col} {op} {p}"))
        }
    }
}

fn push_string_param(params: &mut Vec<SqlParam>, value: &str) {
    params.push(SqlParam::String(value.to_string()));
}

fn placeholder_at(dialect: &dyn Dialect, len: usize) -> String {
    placeholder_label(dialect, len.saturating_sub(1))
}

fn parse_in_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().trim_matches('\'').trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn build_order_by(dialect: &dyn Dialect, sort: &SortModel) -> String {
    sort.keys
        .iter()
        .map(|k| {
            let dir = match k.direction {
                SortDirection::Asc => "ASC",
                SortDirection::Desc => "DESC",
            };
            format!("{} {dir}", dialect.quote_identifier(&k.column))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::filter::{FilterCombine, FilterTerm};
    use crate::query::sort::{SortDirection, SortKey};
    use wisp_drivers::{MysqlDialect, PostgresDialect};

    #[test]
    fn injection_attempt_is_bound_not_concatenated() {
        let mut query = TableDataQuery::for_table("orders");
        query.filter.terms.push(FilterTerm {
            column: "status".into(),
            operator: FilterOperator::Eq,
            value: "'; DROP TABLE orders; --".into(),
            value_to: String::new(),
        });
        let built = build_table_select(&PostgresDialect, &query, 100, 0);
        assert!(built.sql.contains("$1"));
        assert!(!built.sql.contains("DROP TABLE"));
        assert_eq!(built.params.len(), 1);
        assert_eq!(
            built.params[0],
            SqlParam::String("'; DROP TABLE orders; --".into())
        );
    }

    #[test]
    fn postgres_ilike_and_order() {
        let mut query = TableDataQuery::for_table("customers");
        query.filter.terms.push(FilterTerm {
            column: "email".into(),
            operator: FilterOperator::ILike,
            value: "%@example.com".into(),
            value_to: String::new(),
        });
        query.sort.keys.push(SortKey {
            column: "created_at".into(),
            direction: SortDirection::Desc,
        });
        let built = build_table_select(&PostgresDialect, &query, 300, 600);
        assert!(built.sql.contains("ILIKE $1"));
        assert!(built.sql.contains("\"created_at\" DESC"));
        assert!(built.sql.contains("LIMIT 300 OFFSET 600"));
    }

    #[test]
    fn mysql_uses_like_not_ilike() {
        let mut query = TableDataQuery::for_table("customers");
        query.filter.terms.push(FilterTerm {
            column: "email".into(),
            operator: FilterOperator::ILike,
            value: "%a%".into(),
            value_to: String::new(),
        });
        let built = build_table_select(&MysqlDialect, &query, 50, 0);
        assert!(built.sql.contains("LIKE ?"));
        assert!(!built.sql.contains("ILIKE"));
    }

    #[test]
    fn and_or_combine() {
        let mut query = TableDataQuery::for_table("t");
        query.filter.combine = FilterCombine::Or;
        query.filter.terms.push(FilterTerm {
            column: "a".into(),
            operator: FilterOperator::Eq,
            value: "1".into(),
            value_to: String::new(),
        });
        query.filter.terms.push(FilterTerm {
            column: "b".into(),
            operator: FilterOperator::IsNull,
            value: String::new(),
            value_to: String::new(),
        });
        let built = build_table_select(&PostgresDialect, &query, 10, 0);
        assert!(built.sql.contains(" OR "));
        assert!(built.sql.contains("IS NULL"));
    }
}
