//! SSH bastion helpers for database drivers (LUM-019).

use crate::DriverError;
use wisp_store::SshSettings;
use wisp_transport::{
    open_direct_tcpip, open_local_forward, SshConnectParams, SshError, SshSecrets, SshSession,
    SshTunnelStream,
};

#[derive(Debug, Clone, Default)]
pub struct DriverSshSecrets {
    pub password: Option<String>,
    pub key_passphrase: Option<String>,
}

pub fn ssh_allow_tofu() -> bool {
    std::env::var_os("WISP_SSH_TOFU").is_some()
}

pub fn connect_params(
    settings: &SshSettings,
    secrets: &DriverSshSecrets,
    target_host: &str,
    target_port: u16,
) -> SshConnectParams {
    SshConnectParams {
        settings: settings.clone(),
        secrets: SshSecrets {
            password: secrets.password.clone(),
            key_passphrase: secrets.key_passphrase.clone(),
        },
        target_host: target_host.to_string(),
        target_port,
        allow_tofu: ssh_allow_tofu(),
    }
}

pub async fn postgres_tunnel(
    settings: &SshSettings,
    secrets: &DriverSshSecrets,
    target_host: &str,
    target_port: u16,
) -> Result<SshTunnelStream, SshError> {
    open_direct_tcpip(&connect_params(
        settings,
        secrets,
        target_host,
        target_port,
    ))
    .await
}

pub async fn mysql_local_forward(
    settings: &SshSettings,
    secrets: &DriverSshSecrets,
    target_host: &str,
    target_port: u16,
) -> Result<(u16, SshSession), SshError> {
    open_local_forward(&connect_params(
        settings,
        secrets,
        target_host,
        target_port,
    ))
    .await
}

pub fn map_ssh_error(err: SshError) -> DriverError {
    match err {
        SshError::HostKeyChanged { expected, actual } => DriverError::user(
            "SSH host key changed",
            format!("expected {expected}, server presented {actual}"),
        ),
        SshError::UnknownHostKey { fingerprint } => DriverError::user(
            "SSH host key unknown",
            format!("fingerprint {fingerprint} (enable trust on first use to accept)"),
        ),
        SshError::AuthFailed => DriverError::user("SSH authentication failed", "check credentials"),
        other => DriverError::user("SSH tunnel failed", other.to_string()),
    }
}
