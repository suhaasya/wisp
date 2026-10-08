//! `connections.toml` schema (versioned, no secrets).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CONNECTIONS_VERSION: u32 = 1;

/// Stable connection identity (UUID v7 for creation-time ordering).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConnectionId(pub Uuid);

impl ConnectionId {
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7())
    }
}

impl std::fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionsFile {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub folders: Vec<ConnectionFolder>,
    #[serde(default)]
    pub profiles: Vec<ConnectionProfile>,
}

fn default_version() -> u32 {
    CONNECTIONS_VERSION
}

impl Default for ConnectionsFile {
    fn default() -> Self {
        Self {
            version: CONNECTIONS_VERSION,
            folders: Vec::new(),
            profiles: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionFolder {
    pub id: ConnectionId,
    pub name: String,
    pub order: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub id: ConnectionId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<ConnectionId>,
    pub engine: ConnectionEngine,
    pub transport: TransportKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    #[serde(default)]
    pub ssl: SslSettings,
    #[serde(default)]
    pub ssh: SshSettings,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
    #[serde(default)]
    pub env_tag: EnvironmentTag,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub safe_mode: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_unix: Option<i64>,
}

impl ConnectionProfile {
    pub fn new(name: impl Into<String>, engine: ConnectionEngine) -> Self {
        Self {
            id: ConnectionId::new_v7(),
            name: name.into(),
            folder_id: None,
            engine,
            transport: TransportKind::Tcp,
            host: None,
            port: None,
            user: None,
            database: None,
            ssl: SslSettings::default(),
            ssh: SshSettings::default(),
            options: BTreeMap::new(),
            env_tag: EnvironmentTag::default(),
            read_only: false,
            safe_mode: false,
            colour: None,
            last_used_unix: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionEngine {
    #[default]
    PostgreSql,
    MySql,
    MariaDb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    #[default]
    Tcp,
    UnixSocket,
    NamedPipe,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentTag {
    #[default]
    Development,
    Staging,
    Production,
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SslTrustStore {
    #[default]
    System,
    CustomCa,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SslSettings {
    #[serde(default)]
    pub mode: SslMode,
    #[serde(default)]
    pub trust: SslTrustStore,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_cert_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_key_file: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SslMode {
    #[default]
    Disable,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SshAuthMethod {
    #[default]
    Agent,
    Password,
    PublicKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SshSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default)]
    pub auth: SshAuthMethod,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_file: Option<String>,
    #[serde(default)]
    pub use_agent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_host: Option<String>,
}
