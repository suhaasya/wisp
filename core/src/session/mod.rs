//! Live connection sessions (LUM-020).

mod config;
mod driver;
mod error;
mod manager;
mod policy;
mod state;

pub use config::SessionRuntimeConfig;
pub use driver::SessionSecrets;
pub use error::SessionError;
pub use manager::{SessionManager, SessionOpenSpec};
pub use policy::{check_write_allowed, sql_looks_like_write};
pub use state::{SessionEvent, SessionPhase, SessionSnapshot};
