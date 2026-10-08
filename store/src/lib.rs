//! Local configuration, credential storage, and command/query history.
//!
//! Owns durable and semi-durable app data on disk or OS keychains. Must not depend on
//! drivers, transport, UI, or session orchestration in `wisp-core` (avoid cycles).

pub mod window;

/// Placeholder until store implementations land in later milestones.
pub const CRATE_MARKER: &str = "wisp-store";

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(super::CRATE_MARKER, "wisp-store");
    }
}
