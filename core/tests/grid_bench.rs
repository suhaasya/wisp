//! Synthetic 1M-row scroll benchmark (LUM-024 / LUM-004).

use wisp_core::bench_scroll_1m;

#[test]
fn grid_scroll_1m_p99_under_budget() {
    let result = bench_scroll_1m();
    assert_eq!(result.row_count, 1_000_000);
    assert!(
        result.p99_step_us < 50_000,
        "p99 viewport step {} µs exceeds 50ms budget",
        result.p99_step_us
    );
}
