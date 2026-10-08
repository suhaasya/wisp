//! Workspace tab persistence and RAM harness helpers (LUM-026).

mod bench;
mod session;

pub use bench::{bench_tabs_20, Tabs20BenchResult};
pub use session::{
    SavedWorkspaceTab, SavedWorkspaceTabKind, WorkspaceSessionStore, WorkspaceTabState,
};
