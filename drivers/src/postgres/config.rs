//! PostgreSQL connection settings (plain TCP; TLS in LUM-018).

use std::time::Duration;

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
        cfg
    }
}
