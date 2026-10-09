//! Connection form draft, validation, and profile mapping (LUM-016).

use std::{collections::BTreeMap, fmt};

use wisp_store::{
    ConnectionEngine, ConnectionId, ConnectionProfile, EnvironmentTag, SslMode, SslSettings,
    SslTrustStore, SshAuthMethod, SshSettings,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormTab {
    General,
    Ssh,
    Ssl,
    Advanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormEngine {
    PostgreSql,
    MySql,
    MariaDb,
}

impl FormEngine {
    pub const SEGMENTS: [FormEngine; 3] = [
        FormEngine::PostgreSql,
        FormEngine::MySql,
        FormEngine::MariaDb,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::PostgreSql => "PostgreSQL",
            Self::MySql => "MySQL",
            Self::MariaDb => "MariaDB",
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            Self::PostgreSql => 5432,
            Self::MySql | Self::MariaDb => 3306,
        }
    }

    pub fn to_store(self) -> ConnectionEngine {
        match self {
            Self::PostgreSql => ConnectionEngine::PostgreSql,
            Self::MySql => ConnectionEngine::MySql,
            Self::MariaDb => ConnectionEngine::MariaDb,
        }
    }

    pub fn from_store(engine: ConnectionEngine) -> Self {
        match engine {
            ConnectionEngine::PostgreSql => Self::PostgreSql,
            ConnectionEngine::MySql => Self::MySql,
            ConnectionEngine::MariaDb => Self::MariaDb,
        }
    }
}

/// UI environment picker (maps to [`EnvironmentTag`] on save).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormEnvironment {
    #[default]
    Local,
    Dev,
    Staging,
    Production,
}

impl FormEnvironment {
    pub const ALL: [FormEnvironment; 4] = [
        Self::Local,
        Self::Dev,
        Self::Staging,
        Self::Production,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::Dev => "Dev",
            Self::Staging => "Staging",
            Self::Production => "Production",
        }
    }

    pub fn to_tag(self) -> EnvironmentTag {
        match self {
            Self::Local => EnvironmentTag::Custom("local".into()),
            Self::Dev => EnvironmentTag::Development,
            Self::Staging => EnvironmentTag::Staging,
            Self::Production => EnvironmentTag::Production,
        }
    }

    pub fn from_tag(tag: &EnvironmentTag) -> Self {
        match tag {
            EnvironmentTag::Development => Self::Dev,
            EnvironmentTag::Staging => Self::Staging,
            EnvironmentTag::Production => Self::Production,
            EnvironmentTag::Custom(label) if label.eq_ignore_ascii_case("local") => Self::Local,
            EnvironmentTag::Custom(_) => Self::Dev,
        }
    }

    /// Production enables confirm-before-write (`safe_mode`) by default.
    pub fn default_safe_mode(self) -> bool {
        matches!(self, Self::Production)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FormField {
    Name,
    Host,
    Port,
    User,
    Database,
}

#[derive(Clone)]
pub struct ConnectionFormDraft {
    pub id: Option<ConnectionId>,
    pub engine: FormEngine,
    pub name: String,
    pub folder_id: Option<ConnectionId>,
    pub host: String,
    pub port: String,
    pub user: String,
    pub password: String,
    pub database: String,
    pub save_password: bool,
    pub environment: FormEnvironment,
    pub read_only: bool,
    pub safe_mode: bool,
    pub query_history_enabled: bool,
    pub ssl_mode: SslMode,
    pub ssl_trust: SslTrustStore,
    pub ssl_ca_file: String,
    pub ssl_client_cert_file: String,
    pub ssl_client_key_file: String,
    pub ssh_enabled: bool,
    pub ssh_host: String,
    pub ssh_port: String,
    pub ssh_user: String,
    pub ssh_auth: SshAuthMethod,
    pub ssh_identity_file: String,
    pub ssh_use_agent: bool,
    pub ssh_config_host: String,
    pub ssh_password: String,
    pub ssh_key_passphrase: String,
    pub extra_options: BTreeMap<String, String>,
}

impl fmt::Debug for ConnectionFormDraft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionFormDraft")
            .field("id", &self.id)
            .field("engine", &self.engine)
            .field("name", &self.name)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("user", &self.user)
            .field("password", &"[REDACTED]")
            .field("database", &self.database)
            .field("ssl_mode", &self.ssl_mode)
            .field("extra_options", &self.extra_options)
            .finish_non_exhaustive()
    }
}

impl Default for ConnectionFormDraft {
    fn default() -> Self {
        Self::new_connection()
    }
}

impl ConnectionFormDraft {
    pub fn new_connection() -> Self {
        let engine = FormEngine::PostgreSql;
        Self {
            id: None,
            engine,
            name: String::new(),
            folder_id: None,
            host: String::new(),
            port: engine.default_port().to_string(),
            user: String::new(),
            password: String::new(),
            database: String::new(),
            save_password: true,
            environment: FormEnvironment::default(),
            read_only: false,
            safe_mode: false,
            query_history_enabled: true,
            ssl_mode: SslMode::Disable,
            ssl_trust: SslTrustStore::System,
            ssl_ca_file: String::new(),
            ssl_client_cert_file: String::new(),
            ssl_client_key_file: String::new(),
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: "22".into(),
            ssh_user: String::new(),
            ssh_auth: SshAuthMethod::Agent,
            ssh_identity_file: String::new(),
            ssh_use_agent: true,
            ssh_config_host: String::new(),
            ssh_password: String::new(),
            ssh_key_passphrase: String::new(),
            extra_options: BTreeMap::new(),
        }
    }

    pub fn set_engine(&mut self, engine: FormEngine) {
        let previous_default = self.engine.default_port().to_string();
        if self.port.is_empty() || self.port == previous_default {
            self.port = engine.default_port().to_string();
        }
        self.engine = engine;
    }

    pub fn set_environment(&mut self, env: FormEnvironment) {
        if env == FormEnvironment::Production {
            self.safe_mode = true;
        }
        self.environment = env;
    }

    pub fn from_profile(profile: &ConnectionProfile) -> Self {
        Self {
            id: Some(profile.id),
            engine: FormEngine::from_store(profile.engine),
            name: profile.name.clone(),
            folder_id: profile.folder_id,
            host: profile.host.clone().unwrap_or_default(),
            port: profile.port.map_or_else(
                || FormEngine::from_store(profile.engine).default_port().to_string(),
                |p| p.to_string(),
            ),
            user: profile.user.clone().unwrap_or_default(),
            password: String::new(),
            database: profile.database.clone().unwrap_or_default(),
            save_password: true,
            environment: FormEnvironment::from_tag(&profile.env_tag),
            read_only: profile.read_only,
            safe_mode: profile.safe_mode,
            query_history_enabled: profile.query_history_enabled,
            ssl_mode: profile.ssl.mode,
            ssl_trust: profile.ssl.trust,
            ssl_ca_file: profile.ssl.ca_file.clone().unwrap_or_default(),
            ssl_client_cert_file: profile.ssl.client_cert_file.clone().unwrap_or_default(),
            ssl_client_key_file: profile.ssl.client_key_file.clone().unwrap_or_default(),
            ssh_enabled: profile.ssh.enabled,
            ssh_host: profile.ssh.host.clone().unwrap_or_default(),
            ssh_port: profile
                .ssh
                .port
                .map(|p| p.to_string())
                .unwrap_or_else(|| "22".into()),
            ssh_user: profile.ssh.user.clone().unwrap_or_default(),
            ssh_auth: profile.ssh.auth,
            ssh_identity_file: profile.ssh.identity_file.clone().unwrap_or_default(),
            ssh_use_agent: profile.ssh.use_agent,
            ssh_config_host: profile.ssh.config_host.clone().unwrap_or_default(),
            ssh_password: String::new(),
            ssh_key_passphrase: String::new(),
            extra_options: profile.options.clone(),
        }
    }

    pub fn shows_insecure_ssl_warning(&self) -> bool {
        self.ssl_mode == SslMode::Disable && !wisp_drivers::is_local_host(self.host.trim())
    }

    pub fn ssl_settings(&self) -> SslSettings {
        SslSettings {
            mode: self.ssl_mode,
            trust: self.ssl_trust,
            ca_file: opt_nonempty(&self.ssl_ca_file),
            client_cert_file: opt_nonempty(&self.ssl_client_cert_file),
            client_key_file: opt_nonempty(&self.ssl_client_key_file),
        }
    }

    pub fn ssh_settings(&self) -> SshSettings {
        SshSettings {
            enabled: self.ssh_enabled,
            host: opt_nonempty(&self.ssh_host),
            port: self.ssh_port.trim().parse().ok(),
            user: opt_nonempty(&self.ssh_user),
            auth: self.ssh_auth,
            identity_file: opt_nonempty(&self.ssh_identity_file),
            use_agent: self.ssh_use_agent,
            config_host: opt_nonempty(&self.ssh_config_host),
        }
    }

    pub fn to_profile(&self) -> ConnectionProfile {
        let port = self.port.trim().parse().ok();
        ConnectionProfile {
            id: self.id.unwrap_or_else(ConnectionId::new_v7),
            name: self.name.trim().to_string(),
            folder_id: self.folder_id,
            engine: self.engine.to_store(),
            transport: wisp_store::TransportKind::Tcp,
            host: Some(self.host.trim().to_string()),
            port,
            user: Some(self.user.trim().to_string()),
            database: Some(self.database.trim().to_string()),
            ssl: self.ssl_settings(),
            ssh: self.ssh_settings(),
            options: self.extra_options.clone(),
            env_tag: self.environment.to_tag(),
            read_only: self.read_only,
            safe_mode: self.safe_mode,
            query_history_enabled: self.query_history_enabled,
            colour: None,
            last_used_unix: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FormValidation {
    pub field_errors: BTreeMap<FormField, String>,
}

impl FormValidation {
    pub fn is_valid(&self) -> bool {
        self.field_errors.is_empty()
    }
}

fn opt_nonempty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn validate(draft: &ConnectionFormDraft) -> FormValidation {
    let mut field_errors = BTreeMap::new();
    if draft.name.trim().is_empty() {
        field_errors.insert(FormField::Name, "Name is required.".into());
    }
    if draft.host.trim().is_empty() {
        field_errors.insert(FormField::Host, "Host is required.".into());
    } else if !valid_host(draft.host.trim()) {
        field_errors.insert(
            FormField::Host,
            "Enter a valid hostname or IP address.".into(),
        );
    }
    if !matches!(
        draft.port.trim().parse::<u32>(),
        Ok(p) if (1..=65535).contains(&p)
    ) {
        field_errors.insert(
            FormField::Port,
            "Port must be between 1 and 65535.".into(),
        );
    }
    if draft.user.trim().is_empty() {
        field_errors.insert(FormField::User, "User is required.".into());
    }
    if draft.database.trim().is_empty() {
        field_errors.insert(FormField::Database, "Database is required.".into());
    }
    FormValidation { field_errors }
}

fn valid_host(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    host.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && !host.starts_with('-')
        && !host.ends_with('-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_enables_safe_mode_via_set_environment() {
        let mut draft = ConnectionFormDraft::new_connection();
        draft.set_environment(FormEnvironment::Production);
        assert!(draft.safe_mode);
    }

    #[test]
    fn engine_change_updates_default_port() {
        let mut draft = ConnectionFormDraft::new_connection();
        draft.set_engine(FormEngine::MySql);
        assert_eq!(draft.port, "3306");
    }

    #[test]
    fn validation_catches_bad_port() {
        let mut draft = ConnectionFormDraft::new_connection();
        draft.name = "x".into();
        draft.host = "localhost".into();
        draft.user = "u".into();
        draft.database = "d".into();
        draft.port = "99999".into();
        assert!(validate(&draft).field_errors.contains_key(&FormField::Port));
    }
}
