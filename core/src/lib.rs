//! Session lifecycle, result paging, and query execution orchestration.
//!
//! Owns the domain model for connections, running queries, and presenting result sets.
//! May depend on [`wisp_drivers`] and [`wisp_store`]. Must not depend on UI or the
//! binary crate.

pub mod bridge;
pub mod error;
pub mod grid;
pub mod render;

pub use error::{WispError, WispErrorKind};
pub use bridge::{
    assert_no_block_on_ui, enter_runtime_thread, enter_ui_thread, DbBridge, DbCommandPayload,
    DbEvent, DbEventPayload, DbRuntimeConfig, RequestId,
};

/// Placeholder until core session logic lands in later milestones.
pub const CRATE_MARKER: &str = "wisp-core";

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(super::CRATE_MARKER, "wisp-core");
    }
}
