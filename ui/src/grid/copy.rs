//! Background copy (TSV / CSV / SQL INSERT).

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{App, ClipboardItem};
use wisp_core::{PageSource, PagerError, ResultPager, Viewport};

use super::cell::format_cell_text;
use super::columns::ColumnLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyFormat {
    Tsv,
    Csv,
    SqlInsert { table: &'static str },
}

pub struct GridCopyJob {
    pub task: Option<gpui::Task<()>>,
}

impl Default for GridCopyJob {
    fn default() -> Self {
        Self { task: None }
    }
}

pub fn collect_copy_text<S: PageSource>(
    pager: Rc<RefCell<ResultPager<S>>>,
    layout: ColumnLayout,
    rows: &[u64],
    cols: &[usize],
    format: CopyFormat,
) -> Result<String, PagerError> {
    let mut lines = Vec::new();
    let headers: Vec<_> = cols
        .iter()
        .map(|&i| layout.columns[i].meta.name.as_str())
        .collect();
    match format {
        CopyFormat::Tsv | CopyFormat::Csv => {
            let sep = if matches!(format, CopyFormat::Tsv) { '\t' } else { ',' };
            lines.push(headers.join(&sep.to_string()));
            for &row in rows {
                let mut cells = Vec::new();
                for &col in cols {
                    let mut p = pager.borrow_mut();
                    p.set_viewport(Viewport {
                        first_row: row,
                        visible_rows: 1,
                    })?;
                    let text = p.cell_text(row, col)?;
                    let value = p.cell_value(row, col)?;
                    let type_name = &layout.columns[col].meta.type_name;
                    let display = format_cell_text(text.as_deref(), type_name, value.as_ref());
                    cells.push(display.text);
                }
                lines.push(cells.join(&sep.to_string()));
            }
        }
        CopyFormat::SqlInsert { table } => {
            for &row in rows {
                let mut p = pager.borrow_mut();
                p.set_viewport(Viewport {
                    first_row: row,
                    visible_rows: 1,
                })?;
                let mut vals = Vec::new();
                for &col in cols {
                    let text = p.cell_text(row, col)?;
                    let value = p.cell_value(row, col)?;
                    let type_name = &layout.columns[col].meta.type_name;
                    let display = format_cell_text(text.as_deref(), type_name, value.as_ref());
                    vals.push(sql_literal(&display.text));
                }
                lines.push(format!(
                    "INSERT INTO {table} ({}) VALUES ({});",
                    headers.join(", "),
                    vals.join(", ")
                ));
            }
        }
    }
    Ok(lines.join("\n"))
}

pub fn collect_copy_text_sync<S: PageSource>(
    pager: Rc<RefCell<ResultPager<S>>>,
    layout: ColumnLayout,
    rows: &[u64],
    cols: &[usize],
    format: CopyFormat,
    cx: &mut App,
) -> Result<(), PagerError> {
    let text = collect_copy_text(pager, layout, rows, cols, format)?;
    cx.write_to_clipboard(ClipboardItem::new_string(text));
    Ok(())
}

fn sql_literal(text: &str) -> String {
    if text == "NULL" {
        "NULL".into()
    } else {
        format!("'{}'", text.replace('\'', "''"))
    }
}
