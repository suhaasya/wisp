//! Low-level connection transports (TLS, SSH tunnels, plain TCP).
//!
//! Owns wire-level connection setup and secure channel primitives. Must not depend on
//! database drivers, session logic, UI, or persistent storage.

/// Placeholder until transport implementations land in later milestones.
pub const CRATE_MARKER: &str = "wisp-transport";

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(super::CRATE_MARKER, "wisp-transport");
    }
}
