//! Version migrations for `connections.toml`.

use super::schema::{ConnectionsFile, CONNECTIONS_VERSION};

pub fn migrate(mut file: ConnectionsFile) -> ConnectionsFile {
    if file.version == 0 {
        file.version = CONNECTIONS_VERSION;
    }
    if file.version < CONNECTIONS_VERSION {
        file.version = CONNECTIONS_VERSION;
    }
    file
}
