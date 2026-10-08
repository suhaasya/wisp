//! PostgreSQL connection settings (TCP + optional TLS).

use std::time::Duration;

use wisp_store::{SshSettings, SslSettings};

use crate::ssh_tunnel::DriverSshSecrets;

/// TCP connection parameters for [`super::PostgresDriver`].
#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub database: String,
    /// When `None`, trust auth or `.pgpass` is used.
    pub password: Option<String>,
    pub statement_timeout: Option<Duration>,
    pub application_name: String,
    pub ssl: SslSettings,
    pub ssh: SshSettings,
    pub ssh_secrets: DriverSshSecrets,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 5432,
            user: "postgres".into(),
            database: "postgres".into(),
            password: None,
            statement_timeout: Some(Duration::from_secs(30)),
            application_name: "wisp".into(),
            ssl: SslSettings::default(),
            ssh: SshSettings::default(),
            ssh_secrets: DriverSshSecrets::default(),
        }
    }
}

impl PostgresConfig {
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(host) = std::env::var("PGHOST") {
            cfg.host = host;
        }
        if let Ok(port) = std::env::var("PGPORT") {
            if let Ok(port) = port.parse() {
                cfg.port = port;
            }
        }
        if let Ok(user) = std::env::var("PGUSER") {
            cfg.user = user;
        }
        if let Ok(db) = std::env::var("PGDATABASE") {
            cfg.database = db;
        }
        if let Ok(password) = std::env::var("PGPASSWORD") {
            cfg.password = Some(password);
        }
        if std::env::var_os("WISP_SSH_IT").is_some() {
            cfg.ssh.enabled = true;
            cfg.ssh.host = std::env::var("WISP_SSH_HOST").ok();
            cfg.ssh.port = std::env::var("WISP_SSH_PORT")
                .ok()
                .and_then(|p| p.parse().ok());
            cfg.ssh.user = std::env::var("WISP_SSH_USER").ok();
            if let Ok(auth) = std::env::var("WISP_SSH_AUTH") {
                cfg.ssh.auth = match auth.as_str() {
                    "password" => wisp_store::SshAuthMethod::Password,
                    "public_key" => wisp_store::SshAuthMethod::PublicKey,
                    _ => wisp_store::SshAuthMethod::Agent,
                };
            }
            cfg.ssh.use_agent = std::env::var_os("WISP_SSH_USE_AGENT").is_some();
            cfg.ssh.identity_file = std::env::var("WISP_SSH_IDENTITY_FILE").ok();
            if let Ok(pw) = std::env::var("WISP_SSH_PASSWORD") {
                cfg.ssh_secrets.password = Some(pw);
            }
            if let Ok(pp) = std::env::var("WISP_SSH_KEY_PASSPHRASE") {
                cfg.ssh_secrets.key_passphrase = Some(pp);
            }
        }
        cfg
    }
}
