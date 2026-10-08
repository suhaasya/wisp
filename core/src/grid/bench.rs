//! Headless scroll benchmark for LUM-004 / LUM-024 (pager + cell access).

use std::time::Instant;

use wisp_drivers::Value;

use crate::pager::{
    FetchStrategy, PagerConfig, ResultPager, SequentialPageSource, Viewport,
};

#[derive(Debug, Clone, Copy)]
pub struct GridScrollBenchResult {
    pub row_count: u64,
    pub viewport_steps: u32,
    pub p99_step_us: u64,
    pub total_ms: u64,
}

/// Simulates scrolling a 1M × 20 grid by advancing the pager viewport and reading cells.
pub fn bench_scroll_1m() -> GridScrollBenchResult {
    const ROWS: u64 = 1_000_000;
    const COLS: usize = 19;
    let source = SequentialPageSource::new(ROWS, COLS);
    let mut pager =
        ResultPager::new(source, FetchStrategy::Offset, PagerConfig::default()).expect("pager");
    let visible = 40u32;
    let step = 25u32;
    let mut samples = Vec::new();
    let start = Instant::now();
    let mut row = 0u64;
    while row < ROWS {
        let t0 = Instant::now();
        pager
            .set_viewport(Viewport {
                first_row: row,
                visible_rows: visible,
            })
            .expect("viewport");
        for i in 0..visible {
            let r = row + i as u64;
            if r >= ROWS {
                break;
            }
            let _ = pager.cell_value(r, 0).expect("cell");
            if let Some(Value::Text(_)) = pager.cell_value(r, 1).expect("cell") {
                // touch second column
            }
        }
        samples.push(t0.elapsed().as_micros() as u64);
        row = row.saturating_add(step as u64);
    }
    samples.sort_unstable();
    let p99_ix = samples.len().saturating_mul(99).saturating_div(100);
    GridScrollBenchResult {
        row_count: ROWS,
        viewport_steps: samples.len() as u32,
        p99_step_us: samples.get(p99_ix).copied().unwrap_or(0),
        total_ms: start.elapsed().as_millis() as u64,
    }
}
