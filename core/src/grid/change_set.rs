//! Staged table edits (LUM-028). Nothing here touches the database.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Primary key values (column name → display text).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrimaryKey(pub Vec<(String, String)>);

impl PrimaryKey {
    pub fn single(column: impl Into<String>, value: impl Into<String>) -> Self {
        Self(vec![(column.into(), value.into())])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RowKey {
    Pk(PrimaryKey),
    Insert(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellEdit {
    Set(String),
    Null,
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertedRow {
    pub temp_id: u64,
    pub cells: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectiveCell {
    Base(String),
    Value(String),
    Null,
    Default,
}

/// Staged edits keyed by primary key (or temp id for new rows).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableChangeSet {
    pub pk_columns: Vec<String>,
    cell_edits: HashMap<RowKey, HashMap<String, CellEdit>>,
    deleted: HashSet<RowKey>,
    inserts: Vec<InsertedRow>,
    next_temp_id: u64,
}

impl TableChangeSet {
    pub fn with_pk_columns(columns: Vec<String>) -> Self {
        Self {
            pk_columns: columns,
            ..Default::default()
        }
    }

    pub fn is_editable(&self) -> bool {
        !self.pk_columns.is_empty()
    }

    pub fn staged_count(&self) -> u32 {
        let mut n = self
            .deleted
            .iter()
            .filter(|k| matches!(k, RowKey::Pk(_)))
            .count() as u32;
        n += self.inserts.len() as u32;
        for (key, cols) in &self.cell_edits {
            if self.deleted.contains(key) {
                continue;
            }
            if matches!(key, RowKey::Pk(_)) {
                n += cols.len() as u32;
            }
        }
        for ins in &self.inserts {
            n += ins.cells.len() as u32;
        }
        n
    }

    pub fn inserts(&self) -> &[InsertedRow] {
        &self.inserts
    }

    pub fn is_deleted(&self, key: &RowKey) -> bool {
        self.deleted.contains(key)
    }

    pub fn row_state(&self, key: &RowKey) -> RowStage {
        if self.deleted.contains(key) {
            return RowStage::Deleted;
        }
        match key {
            RowKey::Insert(_) => RowStage::Inserted,
            RowKey::Pk(_) => {
                if self.cell_edits.get(key).is_some_and(|m| !m.is_empty()) {
                    RowStage::Modified
                } else {
                    RowStage::Unchanged
                }
            }
        }
    }

    pub fn cell_state(&self, key: &RowKey, column: &str) -> CellStage {
        if self.deleted.contains(key) {
            return CellStage::DeletedRow;
        }
        if let Some(edits) = self.cell_edits.get(key) {
            if let Some(edit) = edits.get(column) {
                return CellStage::Edited(edit.clone());
            }
        }
        if let RowKey::Insert(temp) = key {
            if let Some(row) = self.inserts.iter().find(|r| r.temp_id == *temp) {
                if row.cells.contains_key(column) {
                    return CellStage::Edited(CellEdit::Set(
                        row.cells.get(column).cloned().unwrap_or_default(),
                    ));
                }
            }
            return CellStage::InsertedEmpty;
        }
        CellStage::Clean
    }

    pub fn effective_cell(
        &self,
        key: &RowKey,
        column: &str,
        base: Option<&str>,
    ) -> EffectiveCell {
        if self.deleted.contains(key) {
            return EffectiveCell::Base(base.unwrap_or("").to_string());
        }
        if let Some(edits) = self.cell_edits.get(key) {
            if let Some(edit) = edits.get(column) {
                return match edit {
                    CellEdit::Set(s) => EffectiveCell::Value(s.clone()),
                    CellEdit::Null => EffectiveCell::Null,
                    CellEdit::Default => EffectiveCell::Default,
                };
            }
        }
        if let RowKey::Insert(temp) = key {
            if let Some(row) = self.inserts.iter().find(|r| r.temp_id == *temp) {
                if let Some(v) = row.cells.get(column) {
                    return EffectiveCell::Value(v.clone());
                }
            }
        }
        match base {
            Some(s) => EffectiveCell::Base(s.to_string()),
            None => EffectiveCell::Null,
        }
    }

    pub fn stage_cell(&mut self, key: RowKey, column: impl Into<String>, edit: CellEdit) {
        if self.deleted.contains(&key) {
            return;
        }
        if let RowKey::Insert(temp) = &key {
            let column = column.into();
            let edit_value = match &edit {
                CellEdit::Set(s) => Some(s.clone()),
                CellEdit::Null => Some(String::new()),
                CellEdit::Default => Some(String::new()),
            };
            if let Some(row) = self.inserts.iter_mut().find(|r| r.temp_id == *temp) {
                if let Some(v) = edit_value {
                    if edit == CellEdit::Null || edit == CellEdit::Default {
                        row.cells.remove(&column);
                    } else {
                        row.cells.insert(column, v);
                    }
                }
            }
            return;
        }
        let column = column.into();
        if matches!(edit, CellEdit::Set(_) | CellEdit::Null | CellEdit::Default) {
            self.cell_edits
                .entry(key)
                .or_default()
                .insert(column, edit);
        }
    }

    pub fn revert_cell(&mut self, key: &RowKey, column: &str) {
        if let RowKey::Insert(temp) = key {
            if let Some(row) = self.inserts.iter_mut().find(|r| r.temp_id == *temp) {
                row.cells.remove(column);
            }
            return;
        }
        if let Some(map) = self.cell_edits.get_mut(key) {
            map.remove(column);
            if map.is_empty() {
                self.cell_edits.remove(key);
            }
        }
    }

    pub fn delete_row(&mut self, key: RowKey) {
        if let RowKey::Insert(temp) = &key {
            self.inserts.retain(|r| r.temp_id != *temp);
            self.cell_edits.remove(&key);
            self.deleted.remove(&key);
            return;
        }
        self.deleted.insert(key.clone());
        self.cell_edits.remove(&key);
    }

    pub fn insert_row(&mut self, seed: HashMap<String, String>) -> RowKey {
        let temp_id = self.next_temp_id;
        self.next_temp_id += 1;
        self.inserts.push(InsertedRow {
            temp_id,
            cells: seed,
        });
        RowKey::Insert(temp_id)
    }

    pub fn discard_all(&mut self) {
        self.cell_edits.clear();
        self.deleted.clear();
        self.inserts.clear();
    }

    pub fn deleted_keys(&self) -> impl Iterator<Item = &RowKey> {
        self.deleted.iter()
    }

    pub fn next_temp_id(&self) -> u64 {
        self.next_temp_id
    }

    pub fn set_next_temp_id(&mut self, id: u64) {
        self.next_temp_id = id;
    }

    /// Restore an insert row with a fixed temp id (crash journal).
    pub fn restore_insert_row(&mut self, row: InsertedRow) {
        self.next_temp_id = self.next_temp_id.max(row.temp_id + 1);
        self.inserts.push(row);
    }

    pub fn deleted_pk_keys(&self) -> impl Iterator<Item = &RowKey> {
        self.deleted.iter().filter(|k| matches!(k, RowKey::Pk(_)))
    }

    pub fn updated_rows(&self) -> impl Iterator<Item = (&RowKey, &HashMap<String, CellEdit>)> {
        self.cell_edits
            .iter()
            .filter(|(k, cols)| matches!(k, RowKey::Pk(_)) && !cols.is_empty() && !self.deleted.contains(*k))
    }

    pub fn has_delete(&self) -> bool {
        self.deleted.iter().any(|k| matches!(k, RowKey::Pk(_)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStage {
    Unchanged,
    Modified,
    Inserted,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellStage {
    Clean,
    Edited(CellEdit),
    InsertedEmpty,
    DeletedRow,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(id: &str) -> RowKey {
        RowKey::Pk(PrimaryKey::single("id", id))
    }

    #[test]
    fn edit_then_delete_merges_to_delete() {
        let mut cs = TableChangeSet::with_pk_columns(vec!["id".into()]);
        let key = pk("1");
        cs.stage_cell(key.clone(), "name", CellEdit::Set("x".into()));
        assert_eq!(cs.staged_count(), 1);
        cs.delete_row(key.clone());
        assert!(cs.is_deleted(&key));
        assert_eq!(cs.staged_count(), 1);
        assert!(cs.cell_edits.get(&key).is_none());
    }

    #[test]
    fn insert_then_edit_counts_both() {
        let mut cs = TableChangeSet::with_pk_columns(vec!["id".into()]);
        let key = cs.insert_row(HashMap::from([("name".into(), "a".into())]));
        assert_eq!(cs.staged_count(), 2);
        cs.stage_cell(key.clone(), "qty", CellEdit::Set("3".into()));
        assert_eq!(cs.staged_count(), 3);
    }

    #[test]
    fn insert_delete_removes_staging() {
        let mut cs = TableChangeSet::with_pk_columns(vec!["id".into()]);
        let key = cs.insert_row(HashMap::new());
        cs.delete_row(key.clone());
        assert!(cs.inserts.is_empty());
        assert_eq!(cs.staged_count(), 0);
    }

    #[test]
    fn revert_cell_clears_edit() {
        let mut cs = TableChangeSet::with_pk_columns(vec!["id".into()]);
        let key = pk("9");
        cs.stage_cell(key.clone(), "col", CellEdit::Set("v".into()));
        cs.revert_cell(&key, "col");
        assert_eq!(cs.staged_count(), 0);
    }

    #[test]
    fn discard_all_clears() {
        let mut cs = TableChangeSet::with_pk_columns(vec!["id".into()]);
        cs.stage_cell(pk("1"), "a", CellEdit::Null);
        cs.insert_row(HashMap::new());
        cs.delete_row(pk("2"));
        cs.discard_all();
        assert_eq!(cs.staged_count(), 0);
    }

    #[test]
    fn no_pk_not_editable() {
        let cs = TableChangeSet::default();
        assert!(!cs.is_editable());
    }
}
