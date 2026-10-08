//! Scrollable result grid (hot path — compiled at `opt-level = 3` via crate override).

mod bench;
mod status;

pub use bench::{bench_scroll_1m, GridScrollBenchResult};
pub use crate::pager::{PagerConfig, ResultPager, Viewport};
pub use status::GridStatus;

pub const MODULE_MARKER: &str = "wisp-core-grid";
