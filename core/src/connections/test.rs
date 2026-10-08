//! Background connection test (connect + ping + server info).

use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;
use wisp_drivers::{
    DbDriver, DriverError, DriverSshSecrets, EngineKind, MysqlConfig, MysqlDriver, PostgresConfig,
    PostgresDriver,
};
use wisp_store::{ConnectionEngine, SshSettings, SslSettings};

use super::{
    form::FormEngine,
    test_error::ConnectionTestError,
};

#[derive(Debug, Clone)]
pub struct ConnectionTestSpec {
    pub engine: ConnectionEngine,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    pub ssl: SslSettings,
    pub ssh: SshSettings,
    pub ssh_password: String,
    pub ssh_key_passphrase: String,
}

impl ConnectionTestSpec {
    pub fn from_form(
        engine: FormEngine,
        host: &str,
        port: u16,
        user: &str,
        password: &str,
        database: &str,
        ssl: SslSettings,
        ssh: SshSettings,
        ssh_password: &str,
        ssh_key_passphrase: &str,
    ) -> Self {
        Self {
            engine: engine.to_store(),
            host: host.to_string(),
            port,
            user: user.to_string(),
            password: password.to_string(),
            database: database.to_string(),
            ssl,
            ssh,
            ssh_password: ssh_password.to_string(),
            ssh_key_passphrase: ssh_key_passphrase.to_string(),
        }
    }
}

fn ssh_secrets(spec: &ConnectionTestSpec) -> DriverSshSecrets {
    DriverSshSecrets {
        password: non_empty(&spec.ssh_password),
        key_passphrase: non_empty(&spec.ssh_key_passphrase),
    }
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionTestOutcome {
    pub latency_ms: u64,
    pub version: String,
    pub database: String,
    pub user: String,
    pub tls_summary: String,
}

const TEST_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn run_connection_test(
    spec: ConnectionTestSpec,
    cancel: CancellationToken,
) -> Result<ConnectionTestOutcome, ConnectionTestError> {
    if cancel.is_cancelled() {
        return Err(ConnectionTestError::Cancelled);
    }
    tokio::select! {
        () = cancel.cancelled() => Err(ConnectionTestError::Cancelled),
        res = tokio::time::timeout(TEST_TIMEOUT, perform_test(spec)) => {
            match res {
                Ok(inner) => inner,
                Err(_) => Err(ConnectionTestError::Timeout),
            }
        }
    }
}

async fn perform_test(spec: ConnectionTestSpec) -> Result<ConnectionTestOutcome, ConnectionTestError> {
    let start = Instant::now();
    let handle = tokio::runtime::Handle::try_current()
        .map_err(|e| ConnectionTestError::Other(e.to_string()))?;
    let outcome = tokio::task::spawn_blocking(move || run_blocking_test(spec, handle))
        .await
        .map_err(|e| ConnectionTestError::Other(format!("connection test task: {e}")))??;
    let latency_ms = start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    Ok(ConnectionTestOutcome {
        latency_ms,
        ..outcome
    })
}

fn run_blocking_test(
    spec: ConnectionTestSpec,
    runtime: tokio::runtime::Handle,
) -> Result<ConnectionTestOutcome, ConnectionTestError> {
    if spec.host == "wisp-mock" {
        return Ok(ConnectionTestOutcome {
            latency_ms: 0,
            version: "mock 1.0".into(),
            database: spec.database,
            user: spec.user,
            tls_summary: "Not used (plain TCP)".into(),
        });
    }

    match spec.engine {
        ConnectionEngine::PostgreSql => test_postgres(&spec, runtime),
        ConnectionEngine::MySql | ConnectionEngine::MariaDb => test_mysql(&spec, runtime),
    }
}

fn test_postgres(
    spec: &ConnectionTestSpec,
    runtime: tokio::runtime::Handle,
) -> Result<ConnectionTestOutcome, ConnectionTestError> {
    let config = PostgresConfig {
        host: spec.host.clone(),
        port: spec.port,
        user: spec.user.clone(),
        database: spec.database.clone(),
        password: Some(spec.password.clone()),
        ssl: spec.ssl.clone(),
        ssh: spec.ssh.clone(),
        ssh_secrets: ssh_secrets(spec),
        ..PostgresConfig::default()
    };
    let mut driver = PostgresDriver::new(config, runtime);
    connect_driver(&mut driver)?;
    driver.ping().map_err(map_driver_err)?;
    let info = driver.server_info().map_err(map_driver_err)?;
    driver.close().ok();
    Ok(outcome_from_info(&info))
}

fn test_mysql(
    spec: &ConnectionTestSpec,
    runtime: tokio::runtime::Handle,
) -> Result<ConnectionTestOutcome, ConnectionTestError> {
    let config = MysqlConfig {
        host: spec.host.clone(),
        port: spec.port,
        user: spec.user.clone(),
        database: spec.database.clone(),
        password: Some(spec.password.clone()),
        ssl: spec.ssl.clone(),
        ssh: spec.ssh.clone(),
        ssh_secrets: ssh_secrets(spec),
        ..MysqlConfig::default()
    };
    let mut driver = MysqlDriver::new(config, runtime);
    connect_driver(&mut driver)?;
    driver.ping().map_err(map_driver_err)?;
    let info = driver.server_info().map_err(map_driver_err)?;
    driver.close().ok();
    Ok(outcome_from_info(&info))
}

fn connect_driver(driver: &mut dyn DbDriver) -> Result<(), ConnectionTestError> {
    driver.connect().map_err(map_driver_err)
}

fn map_driver_err(err: DriverError) -> ConnectionTestError {
    match err {
        DriverError::Cancelled => ConnectionTestError::Cancelled,
        DriverError::User { message, detail } => ConnectionTestError::classify(&message, &detail),
        other => ConnectionTestError::Other(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_host_succeeds() {
        let spec = ConnectionTestSpec {
            engine: ConnectionEngine::PostgreSql,
            host: "wisp-mock".into(),
            port: 5432,
            user: "u".into(),
            password: String::new(),
            database: "d".into(),
            ssl: SslSettings::default(),
            ssh: SshSettings::default(),
            ssh_password: String::new(),
            ssh_key_passphrase: String::new(),
        };
        let outcome = run_connection_test(spec, CancellationToken::new())
            .await
            .expect("mock test");
        assert!(outcome.version.contains("mock"));
    }
}

fn outcome_from_info(info: &wisp_drivers::ServerInfo) -> ConnectionTestOutcome {
    let tls_summary = if let Some(tls) = &info.tls {
        tls.summary()
    } else {
        match info.engine {
            EngineKind::Mock => "Not used (mock)".to_string(),
            _ => "Not used (plain TCP)".to_string(),
        }
    };
    ConnectionTestOutcome {
        latency_ms: 0,
        version: info.version.clone(),
        database: info.database.clone(),
        user: info.user.clone(),
        tls_summary,
    }
}
