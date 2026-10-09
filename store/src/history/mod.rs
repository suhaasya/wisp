//! Per-connection query history (JSONL, LUM-034).

mod entry;
mod sanitize;
mod search;
mod writer;

pub use entry::{QueryHistoryEntry, QueryHistoryStatus};
pub use sanitize::sql_for_history;
pub use search::{count_valid_entries, search_history_file};
pub use writer::{append_history_entry, rotate_if_needed, HistoryWriteError, HISTORY_MAX_BYTES};
