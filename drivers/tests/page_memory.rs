//! Heap budget for column-major pages (LUM-011 acceptance).

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use wisp_drivers::{ColumnMeta, PageBuilder};

#[test]
fn page_300_by_10_short_strings_allocation_budget() {
    const ROWS: usize = 300;
    const COLS: usize = 10;
    const STR_LEN: usize = 8;

    let columns: Vec<ColumnMeta> = (0..COLS)
        .map(|c| ColumnMeta::new(format!("col_{c}"), "text"))
        .collect();

    let flat: Vec<String> = (0..ROWS)
        .flat_map(|row| (0..COLS).map(move |col| format!("r{row}c{col}")))
        .collect();

    let _profiler = dhat::Profiler::new_heap();

    let mut builder = PageBuilder::with_capacity(columns, ROWS, ROWS * COLS * STR_LEN);
    for row in 0..ROWS {
        let cells: [&str; COLS] = std::array::from_fn(|col| flat[row * COLS + col].as_str());
        builder.push_text_row_array(cells).expect("row");
    }
    let page = builder.finish();

    assert_eq!(page.row_count, ROWS);
    assert_eq!(page.column_count(), COLS);

    let stats = dhat::HeapStats::get();
    assert!(
        stats.total_blocks <= 14,
        "expected ≤14 heap blocks, got {} (max {} bytes)",
        stats.total_blocks,
        stats.max_bytes
    );
    assert!(
        stats.max_bytes < 512 * 1024,
        "page heap should stay under 512 KiB, got {} bytes",
        stats.max_bytes
    );
}
