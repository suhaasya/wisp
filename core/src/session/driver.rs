//! Open [`DbDriver`] instances from saved profiles.

use std::time::Duration;

use wisp_drivers::{
    DbDriver, DriverError, DriverSshSecrets, MockDriver, MysqlConfig, MysqlDriver, PostgresConfig,
    PostgresDriver,
};
use wisp_store::{ConnectionEngine, ConnectionProfile, SecretKind};

use super::error::SessionError;

#[derive(Debug, Clone, Default)]
pub struct SessionSecrets {
    pub password: Option<String>,
    pub ssh_password: Option<String>,
    pub ssh_key_passphrase: Option<String>,
}

impl SessionSecrets {
    pub fn load(store: &wisp_store::SharedSecretStore, connection_id: &str) -> Self {
        let password = store
            .get(connection_id, SecretKind::Password)
            .ok()
            .map(|s| s.expose_str().to_string());
        let ssh_key_passphrase = store
            .get(connection_id, SecretKind::SshPassphrase)
            .ok()
            .map(|s| s.expose_str().to_string());
        Self {
            password,
            ssh_password: None,
            ssh_key_passphrase,
        }
    }
}

pub fn open_main_driver(
    profile: &ConnectionProfile,
    secrets: &SessionSecrets,
    runtime: tokio::runtime::Handle,
) -> Result<Box<dyn DbDriver>, SessionError> {
    if profile.host.as_deref() == Some("wisp-mock") {
        let mut mock = MockDriver::default();
        mock.connect()
            .map_err(|e| SessionError::Connect(e.to_string()))?;
        return Ok(Box::new(mock));
    }
    match profile.engine {
        ConnectionEngine::PostgreSql => {
            let config = postgres_config(profile, secrets, "wisp");
            let mut driver = PostgresDriver::new(config, runtime);
            driver
                .connect()
                .map_err(|e| SessionError::Connect(e.to_string()))?;
            Ok(Box::new(driver))
        }
        ConnectionEngine::MySql | ConnectionEngine::MariaDb => {
            let config = mysql_config(profile, secrets);
            let mut driver = MysqlDriver::new(config, runtime);
            driver
                .connect()
                .map_err(|e| SessionError::Connect(e.to_string()))?;
            Ok(Box::new(driver))
        }
    }
}

pub fn open_metadata_driver(
    profile: &ConnectionProfile,
    secrets: &SessionSecrets,
    runtime: tokio::runtime::Handle,
) -> Result<Box<dyn DbDriver>, SessionError> {
    if profile.host.as_deref() == Some("wisp-mock") {
        let mut mock = MockDriver::default();
        mock.connect()
            .map_err(|e| SessionError::Connect(e.to_string()))?;
        return Ok(Box::new(mock));
    }
    match profile.engine {
        ConnectionEngine::PostgreSql => {
            let config = postgres_config(profile, secrets, "wisp-metadata");
            let mut driver = PostgresDriver::new(config, runtime);
            driver
                .connect()
                .map_err(|e| SessionError::Connect(e.to_string()))?;
            Ok(Box::new(driver))
        }
        ConnectionEngine::MySql | ConnectionEngine::MariaDb => {
            let config = mysql_config(profile, secrets);
            let mut driver = MysqlDriver::new(config, runtime);
            driver
                .connect()
                .map_err(|e| SessionError::Connect(e.to_string()))?;
            Ok(Box::new(driver))
        }
    }
}

pub fn apply_read_only_session(driver: &mut dyn DbDriver, profile: &ConnectionProfile) -> Result<(), SessionError> {
    if !profile.read_only {
        return Ok(());
    }
    match profile.engine {
        ConnectionEngine::PostgreSql => {
            driver
                .execute("SET default_transaction_read_only = on")
                .map_err(map_query)?;
            driver
                .execute("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY")
                .map_err(map_query)?;
        }
        ConnectionEngine::MySql | ConnectionEngine::MariaDb => {
            driver
                .execute("SET SESSION TRANSACTION READ ONLY")
                .map_err(map_query)?;
        }
    }
    Ok(())
}

fn postgres_config(
    profile: &ConnectionProfile,
    secrets: &SessionSecrets,
    application_name: &str,
) -> PostgresConfig {
    PostgresConfig {
        host: profile.host.clone().unwrap_or_else(|| "127.0.0.1".into()),
        port: profile.port.unwrap_or(5432),
        user: profile.user.clone().unwrap_or_else(|| "postgres".into()),
        database: profile
            .database
            .clone()
            .unwrap_or_else(|| "postgres".into()),
        password: secrets.password.clone(),
        statement_timeout: Some(Duration::from_secs(30)),
        application_name: application_name.into(),
        ssl: profile.ssl.clone(),
        ssh: profile.ssh.clone(),
        ssh_secrets: DriverSshSecrets {
            password: secrets.ssh_password.clone(),
            key_passphrase: secrets.ssh_key_passphrase.clone(),
        },
    }
}

fn mysql_config(profile: &ConnectionProfile, secrets: &SessionSecrets) -> MysqlConfig {
    MysqlConfig {
        host: profile.host.clone().unwrap_or_else(|| "127.0.0.1".into()),
        port: profile.port.unwrap_or(3306),
        user: profile.user.clone().unwrap_or_else(|| "root".into()),
        database: profile
            .database
            .clone()
            .unwrap_or_else(|| "mysql".into()),
        password: secrets.password.clone(),
        ssl: profile.ssl.clone(),
        ssh: profile.ssh.clone(),
        ssh_secrets: DriverSshSecrets {
            password: secrets.ssh_password.clone(),
            key_passphrase: secrets.ssh_key_passphrase.clone(),
        },
        ..MysqlConfig::default()
    }
}

fn map_query(err: DriverError) -> SessionError {
    SessionError::Query(err.to_string())
}
