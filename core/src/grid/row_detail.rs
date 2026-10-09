//! Row detail panel model: truncation, JSON pretty-print, field list (LUM-027).

pub const LARGE_VALUE_THRESHOLD: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowDetailField {
    pub name: String,
    pub type_name: String,
    pub preview: String,
    pub is_null: bool,
    pub is_truncated: bool,
    pub is_pk: bool,
}

pub fn preview_cell_text(full: &str) -> String {
    if full.len() <= LARGE_VALUE_THRESHOLD {
        return full.to_string();
    }
    let mut end = LARGE_VALUE_THRESHOLD;
    while !full.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    format!("{}…", &full[..end])
}

pub fn full_cell_text(table: &str, row: u64, column: &str) -> String {
    if column == "notes" {
        return format!("Customer notes for {table} row {row}. ")
            .repeat(120)
            .chars()
            .take(8192)
            .collect();
    }
    format!("{table}:{column}:{row}")
}

pub fn pretty_format_value(type_name: &str, text: &str) -> String {
    if type_name.contains("json") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
            return serde_json::to_string_pretty(&v).unwrap_or_else(|_| text.to_string());
        }
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_above_4kb() {
        let s = "x".repeat(5000);
        let p = preview_cell_text(&s);
        assert!(p.len() < 5000);
        assert!(p.ends_with('…'));
    }
}
