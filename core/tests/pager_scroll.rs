//! Property-style scroll coverage vs full sequential fetch (LUM-023).

use proptest::prelude::*;
use wisp_core::{
    FetchStrategy, KeysetPlan, PagerConfig, ResultPager, SequentialPageSource, Viewport,
};

const TOTAL: u64 = 100_000;

fn reference_row(id: u64) -> i64 {
    id as i64
}

fn full_fetch_ids(total: u64) -> Vec<i64> {
    (0..total).map(reference_row).collect()
}

proptest! {
    #[test]
    fn random_viewport_windows_match_reference(
        start in 0u64..TOTAL.saturating_sub(500),
        step in 1u64..400u64,
        hops in 1usize..40usize,
    ) {
        let source = SequentialPageSource::new(TOTAL, 2);
        let strategy = FetchStrategy::Keyset(KeysetPlan {
            order_columns: vec!["id".into()],
            primary_key: "id".into(),
        });
        let mut pager = ResultPager::new(source, strategy, PagerConfig::default()).expect("pager");
        let reference = full_fetch_ids(TOTAL);

        let mut row = start;
        for _ in 0..hops {
            let visible = 120u32;
            pager.set_viewport(Viewport { first_row: row, visible_rows: visible }).expect("viewport");
            for offset in 0..visible {
                let global = row.saturating_add(offset as u64);
                if global >= TOTAL {
                    break;
                }
                let id = pager.row_id(global).expect("loaded");
                prop_assert_eq!(id, reference[global as usize]);
            }
            row = row.saturating_add(step).min(TOTAL.saturating_sub(1));
        }
    }

    #[test]
    fn page_boundary_scan_has_no_duplicates_or_gaps(
        pages in 1usize..120usize,
    ) {
        let source = SequentialPageSource::new(TOTAL, 1);
        let mut pager = ResultPager::new(
            source,
            FetchStrategy::Offset,
            PagerConfig::default(),
        )
        .expect("pager");
        let mut collected = Vec::new();
        let rows_per_page = pager.config().rows_per_page as u64;
        for page in 0..pages {
            let first = page as u64 * rows_per_page;
            if first >= TOTAL {
                break;
            }
            pager
                .set_viewport(Viewport {
                    first_row: first,
                    visible_rows: pager.config().rows_per_page,
                })
                .expect("viewport");
            for local in 0..rows_per_page {
                let global = first + local;
                if global >= TOTAL {
                    break;
                }
                collected.push(pager.row_id(global).expect("row"));
            }
        }
        for (i, &id) in collected.iter().enumerate() {
            prop_assert_eq!(id, i as i64);
        }
    }
}
