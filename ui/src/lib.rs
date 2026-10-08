//! GPUI-based widgets, layouts, and interaction for the Wisp client.
//!
//! Owns presentation and input handling wired to core session state. May depend on
//! [`wisp_core`] only among Wisp crates. Must not depend on drivers, transport, store,
//! or the application binary directly.

/// Placeholder until GPUI integration lands in LUM-005.
pub const CRATE_MARKER: &str = "wisp-ui";

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(super::CRATE_MARKER, "wisp-ui");
    }
}
