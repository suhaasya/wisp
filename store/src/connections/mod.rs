//! Saved connection profiles (`connections.toml`, LUM-014).

mod io;
mod migrate;
mod parse;
mod schema;
mod store;

pub use parse::{parse_connections_toml, ConnectionsParseError};
pub use schema::{
    ConnectionEngine, ConnectionFolder, ConnectionId, ConnectionProfile, ConnectionsFile,
    EnvironmentTag, SslMode, SslSettings, SslTrustStore, SshAuthMethod, SshSettings, TransportKind,
    CONNECTIONS_VERSION,
};
pub use store::{ConnectionStore, ConnectionStoreError, ConnectionsLoadError};
