//! Database protocol drivers (PostgreSQL, MySQL, and future engines).
//!
//! Owns engine-specific wire protocol and query execution adapters. May depend on
//! [`wisp_transport`] only among Wisp crates. Must not depend on sessions, UI, or storage.

/// Placeholder until driver implementations land in later milestones.
pub const CRATE_MARKER: &str = "wisp-drivers";

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(super::CRATE_MARKER, "wisp-drivers");
    }
}
