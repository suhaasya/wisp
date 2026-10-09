//! Serialize staged grid edits for crash journal (LUM-035).

use std::collections::HashMap;

use wisp_store::{
    JournalCellEdit, JournalInsertedRow, JournalRowKey, StagedCellEditRow, StagedGridSnapshot,
};

use super::change_set::{CellEdit, InsertedRow, PrimaryKey, RowKey, TableChangeSet};

impl TableChangeSet {
    pub fn to_staged_snapshot(&self) -> StagedGridSnapshot {
        StagedGridSnapshot {
            pk_columns: self.pk_columns.clone(),
            cell_edits: self
                .updated_rows()
                .map(|(key, cols)| StagedCellEditRow {
                    key: row_key_to_journal(key),
                    columns: cols
                        .iter()
                        .map(|(c, e)| (c.clone(), cell_edit_to_journal(e)))
                        .collect(),
                })
                .collect(),
            deleted: self
                .deleted_keys()
                .map(row_key_to_journal)
                .collect(),
            inserts: self
                .inserts()
                .iter()
                .map(|r| JournalInsertedRow {
                    temp_id: r.temp_id,
                    cells: r.cells.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                })
                .collect(),
            next_temp_id: self.next_temp_id(),
        }
    }

    pub fn apply_staged_snapshot(&mut self, snap: &StagedGridSnapshot) {
        self.pk_columns = snap.pk_columns.clone();
        self.discard_all();
        self.set_next_temp_id(snap.next_temp_id);
        for ins in &snap.inserts {
            let cells: HashMap<String, String> = ins.cells.iter().cloned().collect();
            self.restore_insert_row(InsertedRow {
                temp_id: ins.temp_id,
                cells,
            });
        }
        for row in &snap.cell_edits {
            let key = journal_to_row_key(&row.key);
            for (col, edit) in &row.columns {
                self.stage_cell(key.clone(), col.clone(), journal_to_cell_edit(edit));
            }
        }
        for key in &snap.deleted {
            self.delete_row(journal_to_row_key(key));
        }
    }
}

fn row_key_to_journal(key: &RowKey) -> JournalRowKey {
    match key {
        RowKey::Pk(PrimaryKey(cols)) => JournalRowKey::Pk {
            columns: cols.clone(),
        },
        RowKey::Insert(temp_id) => JournalRowKey::Insert { temp_id: *temp_id },
    }
}

fn journal_to_row_key(key: &JournalRowKey) -> RowKey {
    match key {
        JournalRowKey::Pk { columns } => RowKey::Pk(PrimaryKey(columns.clone())),
        JournalRowKey::Insert { temp_id } => RowKey::Insert(*temp_id),
    }
}

fn cell_edit_to_journal(edit: &CellEdit) -> JournalCellEdit {
    match edit {
        CellEdit::Set(v) => JournalCellEdit::Set { value: v.clone() },
        CellEdit::Null => JournalCellEdit::Null,
        CellEdit::Default => JournalCellEdit::Default,
    }
}

fn journal_to_cell_edit(edit: &JournalCellEdit) -> CellEdit {
    match edit {
        JournalCellEdit::Set { value } => CellEdit::Set(value.clone()),
        JournalCellEdit::Null => CellEdit::Null,
        JournalCellEdit::Default => CellEdit::Default,
    }
}
