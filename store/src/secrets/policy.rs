//! Connection-level secret persistence policy.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionSecretPolicy {
    /// Store in OS keychain or encrypted vault (default).
    #[default]
    StoreSecurely,
    /// Never persist; caller must prompt each session.
    AskEveryTime,
}
