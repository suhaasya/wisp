//! Bounded result pager (LUM-023): ring buffer, prefetch, keyset/offset strategies.

mod browse_source;
mod session_sql;
mod config;
mod count;
mod fetch;
mod keyset;
mod pager;
mod queryable;
mod queryable_mock;
mod ring;
mod source;
mod strategy;

pub use config::PagerConfig;
pub use count::RowCount;
pub use fetch::PageFetch;
pub use keyset::tuple_seek_predicate;
pub use pager::{PagerError, ResultPager, Viewport};
pub use browse_source::GridBrowseSource;
pub use session_sql::{SessionSqlFetcher, SessionSqlPageSource};
pub use queryable::QueryableSequentialSource;
pub use queryable_mock::QueryableMockTableSource;
pub use source::{DriverPageSource, PageSource, SequentialPageSource};
pub use strategy::{FetchStrategy, FetchStrategyKind, KeysetCursor, KeysetPlan};
