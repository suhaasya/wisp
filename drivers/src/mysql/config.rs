//! MySQL / MariaDB connection settings (plain TCP; TLS in LUM-018).

use std::time::Duration;

/// TCP connection parameters for [`super::MysqlDriver`].
#[derive(Debug, Clone)]
pub struct MysqlConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub database: String,
    pub password: Option<String>,
    pub statement_timeout: Option<Duration>,
    /// When true, `TINYINT(1)` maps to [`crate::Value::Bool`].
    pub tinyint1_is_bool: bool,
}

impl Default for MysqlConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 3306,
            user: "root".into(),
            database: "mysql".into(),
            password: None,
            statement_timeout: None,
            tinyint1_is_bool: true,
        }
    }
}

impl MysqlConfig {
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(host) = std::env::var("MYSQL_HOST").or_else(|_| std::env::var("MYSQLHOST")) {
            cfg.host = host;
        }
        if let Ok(port) = std::env::var("MYSQL_PORT").or_else(|_| std::env::var("MYSQLPORT")) {
            if let Ok(port) = port.parse() {
                cfg.port = port;
            }
        }
        if let Ok(user) = std::env::var("MYSQL_USER").or_else(|_| std::env::var("MYSQLUSER")) {
            cfg.user = user;
        }
        if let Ok(db) = std::env::var("MYSQL_DATABASE").or_else(|_| std::env::var("MYSQLDATABASE"))
        {
            cfg.database = db;
        }
        if let Ok(password) =
            std::env::var("MYSQL_PASSWORD").or_else(|_| std::env::var("MYSQLPASSWORD"))
        {
            cfg.password = Some(password);
        }
        cfg
    }
}
