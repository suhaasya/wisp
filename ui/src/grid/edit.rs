//! Staged editing helpers (LUM-028).

use wisp_core::{CellEdit, EffectiveCell, RowKey, RowStage, TableChangeSet};

use super::cell::{format_cell_text, CellKind};
use super::columns::ColumnLayout;

pub fn pk_column_names(layout: &ColumnLayout) -> Vec<String> {
    layout
        .columns
        .iter()
        .filter(|c| c.meta.is_pk)
        .map(|c| c.meta.name.clone())
        .collect()
}

pub fn row_key_for_pager_row(
    layout: &ColumnLayout,
    change_set: &TableChangeSet,
    row: u64,
    base_text: impl Fn(u64, usize) -> Option<String>,
) -> Option<RowKey> {
    if change_set.pk_columns.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for name in &change_set.pk_columns {
        let col_ix = layout.columns.iter().position(|c| c.meta.name == *name)?;
        let text = base_text(row, col_ix)?;
        parts.push((name.clone(), text));
    }
    Some(RowKey::Pk(wisp_core::PrimaryKey(parts)))
}

pub fn display_row_stage(
    change_set: &TableChangeSet,
    display_row: u64,
    pager_rows: u64,
    row_key: Option<&RowKey>,
) -> RowStage {
    if display_row >= pager_rows {
        return RowStage::Inserted;
    }
    row_key
        .map(|k| change_set.row_state(k))
        .unwrap_or(RowStage::Unchanged)
}

pub fn effective_to_label(
    effective: &EffectiveCell,
    type_name: &str,
) -> (CellKind, String) {
    match effective {
        EffectiveCell::Null => (CellKind::Null, "NULL".into()),
        EffectiveCell::Default => (CellKind::Text, "DEFAULT".into()),
        EffectiveCell::Value(v) | EffectiveCell::Base(v) => {
            let display = format_cell_text(Some(v.as_str()), type_name, None);
            (display.kind, display.text)
        }
    }
}

pub fn stage_value(change_set: &mut TableChangeSet, key: RowKey, column: &str, raw: &str) {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("null") {
        change_set.stage_cell(key, column, CellEdit::Null);
    } else if trimmed.eq_ignore_ascii_case("default") {
        change_set.stage_cell(key, column, CellEdit::Default);
    } else {
        change_set.stage_cell(key, column, CellEdit::Set(trimmed.to_string()));
    }
}

pub fn editor_kind_for_column(type_name: &str, column: &str) -> EditorKind {
    let ty = type_name.to_ascii_lowercase();
    if ty.contains("bool") {
        return EditorKind::Boolean;
    }
    if ty.contains("json") {
        return EditorKind::Json;
    }
    if ty.contains("enum") || column == "status" {
        return EditorKind::Enum(&["pending", "paid", "shipped", "cancelled"]);
    }
    if ty.contains("timestamp") || ty.contains("datetime") {
        return EditorKind::DateTime;
    }
    if ty.contains("date") && !ty.contains("datetime") {
        return EditorKind::Date;
    }
    if ty.contains("time") && !ty.contains("timestamp") {
        return EditorKind::Time;
    }
    EditorKind::Text
}

#[derive(Debug, Clone, Copy)]
pub enum EditorKind {
    Text,
    Boolean,
    Json,
    Enum(&'static [&'static str]),
    Date,
    Time,
    DateTime,
}
