//! Serializable query page for the DB bridge (LUM-033).

use wisp_drivers::{ColumnMeta, DriverError, Page, PageBuilder, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryPageSnapshot {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<String>>,
}

impl QueryPageSnapshot {
    pub fn from_page(page: &Page) -> Self {
        let mut rows = Vec::with_capacity(page.row_count);
        for row in 0..page.row_count {
            let mut line = Vec::with_capacity(page.column_count());
            for col in 0..page.column_count() {
                line.push(
                    page
                        .text_at(row, col)
                        .map(str::to_string)
                        .or_else(|| cell_display(page, row, col))
                        .unwrap_or_default(),
                );
            }
            rows.push(line);
        }
        Self {
            columns: page.columns.clone(),
            rows,
        }
    }

    pub fn into_page(self) -> Result<Page, DriverError> {
        let mut builder = PageBuilder::new(self.columns);
        for row in &self.rows {
            let refs: Vec<&str> = row.iter().map(String::as_str).collect();
            builder
                .push_text_row(&refs)
                .map_err(|_| DriverError::InvalidPage)?;
        }
        Ok(builder.finish())
    }
}

fn cell_display(page: &Page, row: usize, col: usize) -> Option<String> {
    let value = page.value(row, col)?;
    Some(match value {
        Value::Null => "NULL".into(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    })
}
