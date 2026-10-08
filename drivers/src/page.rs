//! Column-major result pages with one shared arena.

use crate::{
    arena::{BlobRef, StrRef},
    column::ColumnMeta,
    value::Value,
};

/// Shared bump buffer for a single page.
#[derive(Debug, Default)]
pub struct PageArena {
    buf: Vec<u8>,
}

impl PageArena {
    pub fn push_str(&mut self, text: &str) -> StrRef {
        self.push_bytes(text.as_bytes())
    }

    pub fn push_bytes(&mut self, bytes: &[u8]) -> BlobRef {
        let offset = self.buf.len() as u32;
        self.buf.extend_from_slice(bytes);
        BlobRef {
            offset,
            len: bytes.len() as u32,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.buf
    }

    pub fn get(&self, blob: BlobRef) -> &[u8] {
        blob.slice(&self.buf)
    }

    pub fn get_str(&self, text: StrRef) -> Option<&str> {
        text.as_str(&self.buf)
    }
}

/// One page of query results (column-wise storage).
#[derive(Debug)]
pub struct Page {
    pub columns: Vec<ColumnMeta>,
    pub row_count: usize,
    columns_data: Vec<Vec<Value>>,
    arena: PageArena,
}

impl Page {
    pub fn builder(columns: Vec<ColumnMeta>) -> PageBuilder {
        PageBuilder::new(columns)
    }

    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    pub fn value(&self, row: usize, col: usize) -> Option<&Value> {
        self.columns_data.get(col)?.get(row)
    }

    pub fn column_values(&self, col: usize) -> Option<&[Value]> {
        self.columns_data.get(col).map(|c| c.as_slice())
    }

    pub fn arena(&self) -> &PageArena {
        &self.arena
    }

    pub fn text_at(&self, row: usize, col: usize) -> Option<&str> {
        let value = self.value(row, col)?;
        let text = match value {
            Value::Text(r) | Value::Decimal(r) | Value::Json(r) | Value::Unknown(r) => *r,
            _ => return None,
        };
        self.arena.get_str(text)
    }

    pub fn bytes_prefix(&self, row: usize, col: usize) -> Option<&[u8]> {
        let Value::Bytes(preview) = self.value(row, col)? else {
            return None;
        };
        Some(self.arena.get(preview.prefix))
    }
}

/// Builds a [`Page`] row by row while preserving one arena and one buffer per column.
#[derive(Debug)]
pub struct PageBuilder {
    columns: Vec<ColumnMeta>,
    columns_data: Vec<Vec<Value>>,
    arena: PageArena,
    row_count: usize,
}

impl PageBuilder {
    pub fn new(columns: Vec<ColumnMeta>) -> Self {
        Self::with_capacity(columns, 0, 0)
    }

    /// Pre-size column buffers and the arena (one allocation per column + one for arena data).
    pub fn with_capacity(columns: Vec<ColumnMeta>, rows: usize, arena_bytes: usize) -> Self {
        let col_count = columns.len();
        Self {
            columns,
            columns_data: (0..col_count)
                .map(|_| Vec::with_capacity(rows))
                .collect(),
            arena: PageArena {
                buf: Vec::with_capacity(arena_bytes),
            },
            row_count: 0,
        }
    }

    pub fn push_row(&mut self, cells: impl IntoIterator<Item = Value>) -> Result<(), PageError> {
        let mut iter = cells.into_iter();
        for (col, dest) in self.columns_data.iter_mut().enumerate() {
            let cell = iter.next().ok_or(PageError::ColumnCountMismatch {
                expected: self.columns.len(),
                got: col,
            })?;
            dest.push(cell);
        }
        if iter.next().is_some() {
            return Err(PageError::ColumnCountMismatch {
                expected: self.columns.len(),
                got: self.columns.len() + 1,
            });
        }
        self.row_count += 1;
        Ok(())
    }

    pub fn push_text_row(&mut self, texts: &[&str]) -> Result<(), PageError> {
        if texts.len() != self.columns.len() {
            return Err(PageError::ColumnCountMismatch {
                expected: self.columns.len(),
                got: texts.len(),
            });
        }
        let refs: Vec<Value> = texts
            .iter()
            .map(|t| Value::Text(self.arena.push_str(t)))
            .collect();
        self.push_row(refs)
    }

    pub fn push_text_row_array<const N: usize>(
        &mut self,
        texts: [&str; N],
    ) -> Result<(), PageError> {
        if N != self.columns.len() {
            return Err(PageError::ColumnCountMismatch {
                expected: self.columns.len(),
                got: N,
            });
        }
        for (col, text) in texts.into_iter().enumerate() {
            self.columns_data[col].push(Value::Text(self.arena.push_str(text)));
        }
        self.row_count += 1;
        Ok(())
    }

    pub fn arena_mut(&mut self) -> &mut PageArena {
        &mut self.arena
    }

    pub fn finish(self) -> Page {
        Page {
            columns: self.columns,
            row_count: self.row_count,
            columns_data: self.columns_data,
            arena: self.arena,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PageError {
    #[error("expected {expected} columns, got {got}")]
    ColumnCountMismatch { expected: usize, got: usize },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::column::ColumnMeta;

    pub fn fixture_page(rows: usize, cols: usize) -> Page {
        let columns: Vec<_> = (0..cols)
            .map(|c| ColumnMeta::new(format!("col_{c}"), "text"))
            .collect();
        let mut builder = Page::builder(columns);
        for row in 0..rows {
            let texts: Vec<String> = (0..cols).map(|c| format!("r{row}c{c}")).collect();
            let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
            builder.push_text_row(&refs).expect("row");
        }
        builder.finish()
    }

    #[test]
    fn column_major_layout() {
        let page = fixture_page(3, 2);
        assert_eq!(page.text_at(1, 1), Some("r1c1"));
        assert_eq!(page.column_values(0).unwrap().len(), 3);
    }
}
