//! Wisp application entry point.
//!
//! GPUI window bootstrap arrives in LUM-005; this binary only proves the workspace links.

fn main() {
    println!(
        "wisp {} (ui={}, store={}) — hello window placeholder",
        env!("CARGO_PKG_VERSION"),
        wisp_ui::CRATE_MARKER,
        wisp_store::CRATE_MARKER,
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(wisp_ui::CRATE_MARKER, "wisp-ui");
    }
}
