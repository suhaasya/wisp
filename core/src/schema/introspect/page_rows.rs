//! Drain paged driver results into text rows (introspection).

use wisp_drivers::{DbDriver, DriverError, PageRequest, Value};

pub fn fetch_text_rows(
    driver: &mut dyn DbDriver,
    sql: &str,
    page_size: u32,
) -> Result<Vec<Vec<String>>, DriverError> {
    let mut offset = 0u64;
    let mut out = Vec::new();
    loop {
        let page = driver.query_paged(
            sql,
            PageRequest {
                limit: page_size.max(1),
                offset,
            },
        )?;
        if page.row_count == 0 {
            break;
        }
        for row in 0..page.row_count {
            let mut cells = Vec::with_capacity(page.column_count());
            for col in 0..page.column_count() {
                cells.push(value_as_string(&page, row, col));
            }
            out.push(cells);
        }
        if page.row_count < page_size as usize {
            break;
        }
        offset += page.row_count as u64;
    }
    Ok(out)
}

fn value_as_string(page: &wisp_drivers::Page, row: usize, col: usize) -> String {
    if let Some(text) = page.text_at(row, col) {
        return text.to_string();
    }
    match page.value(row, col) {
        Some(Value::Null) => String::new(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Int(i)) => i.to_string(),
        Some(Value::Float(f)) => f.to_string(),
        Some(Value::Bytes(b)) => format!("<{} bytes>", b.total_len),
        Some(v) => format!("{v:?}"),
        None => String::new(),
    }
}
