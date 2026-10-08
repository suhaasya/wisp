//! Evaluate filters/sort on mock rows (same semantics as SQL for SequentialPageSource).

use wisp_drivers::Value;

use super::filter::{FilterCombine, FilterModel, FilterOperator, FilterTerm};
use super::sort::{SortDirection, SortModel};

pub fn row_matches_filter(
    filter: &FilterModel,
    column_value: impl Fn(&str) -> Option<String>,
) -> bool {
    if let Some(raw) = filter.raw_where.as_ref().filter(|s| !s.trim().is_empty()) {
        return eval_raw_where(raw, &column_value);
    }
    let active: Vec<_> = filter
        .terms
        .iter()
        .filter(|t| term_active(t))
        .collect();
    if active.is_empty() {
        return true;
    }
    let results: Vec<bool> = active
        .iter()
        .map(|t| eval_term(t, &column_value))
        .collect();
    match filter.combine {
        FilterCombine::And => results.iter().all(|&b| b),
        FilterCombine::Or => results.iter().any(|&b| b),
    }
}

fn term_active(term: &FilterTerm) -> bool {
    !term.column.is_empty()
        && (!term.operator.needs_value() || !term.value.is_empty() || term.operator == FilterOperator::Between)
}

fn eval_term(term: &FilterTerm, column_value: &impl Fn(&str) -> Option<String>) -> bool {
    let cell = column_value(&term.column);
    match term.operator {
        FilterOperator::IsNull => cell.is_none() || cell.as_deref() == Some("NULL"),
        FilterOperator::IsNotNull => cell.as_deref().is_some_and(|s| s != "NULL"),
        FilterOperator::In => {
            let Some(ref c) = cell else {
                return false;
            };
            term.value
                .split(',')
                .map(|s| s.trim().trim_matches('\'').trim_matches('"'))
                .any(|v| v == c)
        }
        FilterOperator::Between => {
            let Some(ref c) = cell else {
                return false;
            };
            c.as_str() >= term.value.as_str() && c.as_str() <= term.value_to.as_str()
        }
        FilterOperator::Like | FilterOperator::ILike => {
            let Some(ref c) = cell else {
                return false;
            };
            like_match(c, &term.value, term.operator == FilterOperator::ILike)
        }
        FilterOperator::Eq => cell.as_deref() == Some(term.value.as_str()),
        FilterOperator::Ne => cell.as_deref() != Some(term.value.as_str()),
        FilterOperator::Lt => cell.as_deref() < Some(term.value.as_str()),
        FilterOperator::Gt => cell.as_deref() > Some(term.value.as_str()),
        FilterOperator::Lte => cell.as_deref() <= Some(term.value.as_str()),
        FilterOperator::Gte => cell.as_deref() >= Some(term.value.as_str()),
    }
}

fn like_match(haystack: &str, pattern: &str, case_insensitive: bool) -> bool {
    let (h, p) = if case_insensitive {
        (haystack.to_lowercase(), pattern.to_lowercase())
    } else {
        (haystack.to_string(), pattern.to_string())
    };
    if p.contains('%') {
        let parts: Vec<&str> = p.split('%').collect();
        if parts.len() == 2 && parts[0].is_empty() && parts[1].is_empty() {
            return true;
        }
        if parts.len() == 2 && parts[1].is_empty() {
            return h.starts_with(parts[0]);
        }
        if parts.len() == 2 && parts[0].is_empty() {
            return h.ends_with(parts[1]);
        }
        return h.contains(&p.replace('%', ""));
    }
    h == p
}

fn eval_raw_where(raw: &str, column_value: &impl Fn(&str) -> Option<String>) -> bool {
    let lower = raw.to_lowercase();
    if lower.contains("1=1") {
        return true;
    }
    if let Some(col) = extract_eq_column(raw) {
        if let Some(val) = extract_eq_value(raw) {
            return column_value(col).as_deref() == Some(val);
        }
    }
    let _ = column_value;
    false
}

fn extract_eq_column(raw: &str) -> Option<&str> {
    let ix = raw.find('=')?;
    Some(raw[..ix].trim().trim_matches('"').trim_matches('`'))
}

fn extract_eq_value(raw: &str) -> Option<&str> {
    let ix = raw.find('=')?;
    Some(
        raw[ix + 1..]
            .trim()
            .trim_matches('\'')
            .trim_matches('"'),
    )
}

pub fn sort_row_ids(
    mut ids: Vec<u64>,
    sort: &SortModel,
    row_value: impl Fn(u64, &str) -> Option<String>,
) -> Vec<u64> {
    if sort.keys.is_empty() {
        return ids;
    }
    ids.sort_by(|&a, &b| {
        for key in &sort.keys {
            let av = row_value(a, &key.column).unwrap_or_default();
            let bv = row_value(b, &key.column).unwrap_or_default();
            let ord = av.cmp(&bv);
            if ord != std::cmp::Ordering::Equal {
                return match key.direction {
                    SortDirection::Asc => ord,
                    SortDirection::Desc => ord.reverse(),
                };
            }
        }
        a.cmp(&b)
    });
    ids
}

pub fn int_cell(value: &Value) -> String {
    match value {
        Value::Int(n) => n.to_string(),
        Value::Null => "NULL".into(),
        _ => String::new(),
    }
}
