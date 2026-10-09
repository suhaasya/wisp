//! Table-aware mock columns and cell text for browse UI (LUM-027).

use wisp_drivers::{ColumnMeta, DriverError, Page, PageArena, PageBuilder, Value};

use crate::pager::{PageFetch, PageSource, RowCount};

use super::row_detail::{full_cell_text, preview_cell_text, LARGE_VALUE_THRESHOLD};

#[derive(Debug, Clone)]
pub struct MockColumn {
    pub meta: ColumnMeta,
    pub kind: MockCellKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockCellKind {
    Int,
    Text,
    LongText,
    Json,
    Nullable,
}

#[derive(Debug, Clone)]
pub struct MockTableLayout {
    pub table: String,
    pub columns: Vec<MockColumn>,
}

pub fn mock_layout_for_table(table: &str) -> MockTableLayout {
    let table = table.to_string();
    let columns = match table.as_str() {
        "customers" => vec![
            col("id", "bigint", MockCellKind::Int, true),
            col("name", "text", MockCellKind::Text, false),
            col("email", "text", MockCellKind::Text, false),
            col("profile_json", "jsonb", MockCellKind::Json, false),
            col("notes", "text", MockCellKind::LongText, false),
        ],
        "orders" => vec![
            col("id", "bigint", MockCellKind::Int, true),
            col("customer_id", "bigint", MockCellKind::Int, false),
            col("total", "numeric", MockCellKind::Text, false),
            col("status", "enum", MockCellKind::Text, false),
        ],
        "order_items" => vec![
            col("id", "bigint", MockCellKind::Int, true),
            col("order_id", "bigint", MockCellKind::Int, false),
            col("product_id", "bigint", MockCellKind::Int, false),
            col("qty", "int", MockCellKind::Int, false),
        ],
        "products" => vec![
            col("id", "bigint", MockCellKind::Int, true),
            col("sku", "text", MockCellKind::Text, false),
            col("name", "text", MockCellKind::Text, false),
        ],
        "shipments" => vec![
            col("id", "bigint", MockCellKind::Int, true),
            col("order_id", "bigint", MockCellKind::Int, false),
        ],
        "shipment_legs" => vec![
            col("id", "bigint", MockCellKind::Int, true),
            col("shipment_id", "bigint", MockCellKind::Int, false),
            col("order_id", "bigint", MockCellKind::Int, false),
        ],
        _ => generic_columns(9),
    };
    MockTableLayout { table, columns }
}

fn col(name: &str, ty: &str, kind: MockCellKind, pk: bool) -> MockColumn {
    let mut meta = ColumnMeta::new(name, ty);
    meta.is_pk = pk;
    MockColumn { meta, kind }
}

fn generic_columns(n: usize) -> Vec<MockColumn> {
    let mut cols = vec![col("id", "bigint", MockCellKind::Int, true)];
    for i in 0..n {
        cols.push(col(
            &format!("col_{i}"),
            "text",
            MockCellKind::Text,
            false,
        ));
    }
    cols
}

/// Cell payload as display text (`None` = SQL NULL).
pub fn mock_cell_display(table: &str, row: u64, column: &MockColumn) -> Option<String> {
    let name = column.meta.name.as_str();
    match column.kind {
        MockCellKind::Int => {
            let v = match (table, name) {
                ("orders", "customer_id") => (row % 50) + 1,
                ("order_items", "order_id") => (row % 200) + 1,
                ("order_items", "product_id") => (row % 30) + 1,
                ("shipments", "order_id") => (row % 100) + 1,
                ("shipment_legs", "shipment_id") => (row % 40) + 1,
                ("shipment_legs", "order_id") => (row % 100) + 1,
                (_, "id") => row,
                _ => row.wrapping_mul(3).wrapping_add(7),
            };
            Some(v.to_string())
        }
        MockCellKind::Nullable if row % 17 == 0 => None,
        MockCellKind::Json => Some(format!(
            r#"{{"row":{row},"tags":["a","b"],"meta":{{"n":{}}}}}"#,
            row % 5
        )),
        MockCellKind::LongText => Some(full_cell_text(table, row, name)),
        MockCellKind::Text | MockCellKind::Nullable => {
            if table == "orders" && name == "status" {
                let options = ["pending", "paid", "shipped", "cancelled"];
                return Some(options[(row as usize) % options.len()].into());
            }
            Some(format!("{table}:{name}:{row}"))
        }
    }
}

pub fn mock_cell_value(
    table: &str,
    row: u64,
    column: &MockColumn,
    arena: &mut PageArena,
) -> Value {
    match mock_cell_display(table, row, column) {
        None => Value::Null,
        Some(text) if column.kind == MockCellKind::Json => Value::Json(arena.push_str(&text)),
        Some(text) => Value::Text(arena.push_str(&text)),
    }
}

pub fn mock_cell_preview(table: &str, row: u64, column: &MockColumn) -> String {
    match mock_cell_display(table, row, column) {
        None => "NULL".into(),
        Some(full) => preview_cell_text(&full),
    }
}

pub fn mock_cell_needs_lazy_fetch(column: &MockColumn, preview: &str) -> bool {
    matches!(column.kind, MockCellKind::LongText | MockCellKind::Json)
        || preview.ends_with('…')
        || preview.len() >= LARGE_VALUE_THRESHOLD
}

/// Page source with shop-style columns and deterministic cell payloads.
#[derive(Debug, Clone)]
pub struct MockTablePageSource {
    layout: MockTableLayout,
    pub total_rows: u64,
}

impl MockTablePageSource {
    pub fn new(table: impl Into<String>, total_rows: u64) -> Self {
        let table = table.into();
        Self {
            layout: mock_layout_for_table(&table),
            total_rows,
        }
    }

    pub fn layout(&self) -> &MockTableLayout {
        &self.layout
    }
}

impl PageSource for MockTablePageSource {
    fn columns(&self) -> Vec<ColumnMeta> {
        self.layout.columns.iter().map(|c| c.meta.clone()).collect()
    }

    fn row_count_hint(&mut self) -> RowCount {
        RowCount::Exact(self.total_rows)
    }

    fn fetch_count_exact(&mut self) -> Result<u64, DriverError> {
        Ok(self.total_rows)
    }

    fn fetch_page(&mut self, request: PageFetch) -> Result<Page, DriverError> {
        let (offset, limit) = match request {
            PageFetch::Offset { offset, limit } => (offset, limit),
            _ => {
                return Err(DriverError::user(
                    "Unsupported fetch",
                    "mock table source",
                ));
            }
        };
        let mut builder = PageBuilder::new(self.columns());
        let end = offset.saturating_add(limit as u64).min(self.total_rows);
        for row in offset..end {
            let mut cells = Vec::with_capacity(self.layout.columns.len());
            for col in &self.layout.columns {
                let arena = builder.arena_mut();
                cells.push(mock_cell_value(&self.layout.table, row, col, arena));
            }
            builder.push_row(cells).map_err(|_| DriverError::InvalidPage)?;
        }
        Ok(builder.finish())
    }
}
