//! Headless 20-tab RAM scenario (LUM-026): one hot tab, rest evicted.

use std::cell::RefCell;
use std::rc::Rc;

use crate::pager::{
    FetchStrategy, GridBrowseSource, PagerConfig, QueryableSequentialSource, ResultPager,
    SequentialPageSource, Viewport,
};
use crate::query::TableDataQuery;

#[derive(Debug, Clone, Copy)]
pub struct Tabs20BenchResult {
    pub tab_count: u32,
    pub hot_loaded_bytes: usize,
    pub background_loaded_bytes: usize,
}

/// Simulates 20 table tabs: load viewport on tab 0, evict pages on the other 19.
pub fn bench_tabs_20() -> Tabs20BenchResult {
    const TABS: u32 = 20;
    let mut pagers: Vec<Rc<RefCell<ResultPager<GridBrowseSource>>>> = Vec::new();
    for i in 0..TABS {
        let query = TableDataQuery::for_table(format!("bench_t{i}"));
        let inner = SequentialPageSource::new(50_000, 9);
        let source =
            GridBrowseSource::Sequential(QueryableSequentialSource::new(inner, query));
        let pager = ResultPager::new(source, FetchStrategy::Offset, PagerConfig::default())
            .expect("pager");
        pagers.push(Rc::new(RefCell::new(pager)));
    }
    {
        let mut hot = pagers[0].borrow_mut();
        hot.set_viewport(Viewport {
            first_row: 0,
            visible_rows: 40,
        })
        .expect("viewport");
    }
    for pager in pagers.iter().skip(1) {
        pager.borrow_mut().unload_pages();
    }
    let hot_loaded = pagers[0].borrow().loaded_bytes();
    let background_loaded: usize = pagers.iter().skip(1).map(|p| p.borrow().loaded_bytes()).sum();
    Tabs20BenchResult {
        tab_count: TABS,
        hot_loaded_bytes: hot_loaded,
        background_loaded_bytes: background_loaded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_tabs_evict_pages() {
        let result = bench_tabs_20();
        assert_eq!(result.tab_count, 20);
        assert_eq!(result.background_loaded_bytes, 0);
        assert!(result.hot_loaded_bytes > 0);
    }
}
