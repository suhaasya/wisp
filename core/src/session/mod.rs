//! Live connection sessions (LUM-020).

mod config;
mod driver;
mod error;
mod manager;
mod policy;
mod query_page;
mod script;
mod state;

pub use config::SessionRuntimeConfig;
pub use driver::SessionSecrets;
pub use error::SessionError;
pub use manager::{CommitTransactionOutcome, SessionManager, SessionOpenSpec};
pub use query_page::QueryPageSnapshot;
pub use script::{
    RunScriptReport, ScriptStatement, StatementRunOutcome,
};
pub use policy::{check_write_allowed, sql_looks_like_write};
pub use state::{SessionEvent, SessionPhase, SessionSnapshot};
