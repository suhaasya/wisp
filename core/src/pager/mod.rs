//! Bounded result pager (LUM-023): ring buffer, prefetch, keyset/offset strategies.

mod config;
mod count;
mod fetch;
mod keyset;
mod pager;
mod ring;
mod queryable;
mod source;
mod strategy;

pub use config::PagerConfig;
pub use count::RowCount;
pub use fetch::PageFetch;
pub use keyset::tuple_seek_predicate;
pub use pager::{PagerError, ResultPager, Viewport};
pub use queryable::QueryableSequentialSource;
pub use source::{DriverPageSource, PageSource, SequentialPageSource};
pub use strategy::{FetchStrategy, FetchStrategyKind, KeysetCursor, KeysetPlan};
