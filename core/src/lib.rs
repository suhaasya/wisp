//! Session lifecycle, result paging, and query execution orchestration.
//!
//! Owns the domain model for connections, running queries, and presenting result sets.
//! May depend on [`wisp_drivers`] and [`wisp_store`]. Must not depend on UI or the
//! binary crate.

pub mod grid;
pub mod render;

/// Placeholder until core session logic lands in later milestones.
pub const CRATE_MARKER: &str = "wisp-core";

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(super::CRATE_MARKER, "wisp-core");
    }
}
