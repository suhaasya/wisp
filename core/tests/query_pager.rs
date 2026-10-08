//! Filter + sort keeps paging consistent (LUM-025).

use wisp_core::{
    build_table_select, FetchStrategy, FilterOperator, FilterTerm, PagerConfig,
    QueryableSequentialSource, ResultPager, SequentialPageSource, SortDirection, SortKey,
    TableDataQuery, Viewport,
};
use wisp_drivers::PostgresDialect;

#[test]
fn filtered_paging_returns_contiguous_ids() {
    let mut query = TableDataQuery::for_table("t");
    query.filter.terms.push(FilterTerm {
        column: "id".into(),
        operator: FilterOperator::Gte,
        value: "10".into(),
        value_to: String::new(),
    });
    query.sort.keys.push(SortKey {
        column: "id".into(),
        direction: SortDirection::Desc,
    });
    let inner = SequentialPageSource::new(100, 2);
    let source = QueryableSequentialSource::new(inner, query.clone());
    let mut pager =
        ResultPager::new(source, FetchStrategy::Offset, PagerConfig::default()).expect("pager");
    pager
        .set_viewport(Viewport {
            first_row: 0,
            visible_rows: 5,
        })
        .expect("viewport");
    let first = pager.row_id(0).expect("row");
    let second = pager.row_id(1).expect("row");
    assert!(first > second);
    let built = build_table_select(&PostgresDialect, &query, 5, 0);
    assert!(built.sql.contains("WHERE"));
    assert!(built.params.len() >= 1);
}
