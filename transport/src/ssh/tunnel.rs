//! SSH bastion forward via `direct-tcpip`.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use tokio::sync::Mutex;

use russh::client::{self, AuthResult, Handler};
use russh::keys::agent::AgentIdentity;
use russh::keys::{agent, load_secret_key, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate};
use russh::ChannelStream;
use wisp_store::{SshAuthMethod, SshSettings};

use super::error::SshError;
use super::known_hosts::KnownHosts;
use super::openssh_config::OpenSshConfig;

#[derive(Debug, Clone, Default)]
pub struct SshSecrets {
    pub password: Option<String>,
    pub key_passphrase: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SshConnectParams {
    pub settings: SshSettings,
    pub secrets: SshSecrets,
    pub target_host: String,
    pub target_port: u16,
    pub allow_tofu: bool,
}

/// Keeps the SSH session alive while tunnels or forwards are in use.
#[derive(Clone)]
pub struct SshSession {
    _handle: Arc<Mutex<client::Handle<SshClientHandler>>>,
}

/// Byte stream to the database through an SSH tunnel.
pub struct SshTunnelStream {
    pub stream: ChannelStream<client::Msg>,
    session: SshSession,
}

impl SshTunnelStream {
    pub fn into_parts(self) -> (ChannelStream<client::Msg>, SshSession) {
        (self.stream, self.session)
    }
}

pub async fn open_direct_tcpip(params: &SshConnectParams) -> Result<SshTunnelStream, SshError> {
    let handle = establish_ssh_session(params).await?;
    let shared = Arc::new(Mutex::new(handle));
    let session = SshSession {
        _handle: Arc::clone(&shared),
    };
    let stream = {
        let mut guard = shared.lock().await;
        let channel = guard
            .channel_open_direct_tcpip(
                &params.target_host,
                u32::from(params.target_port),
                "127.0.0.1",
                0,
            )
            .await
            .map_err(|e| SshError::Tunnel(e.to_string()))?;
        channel.into_stream()
    };
    Ok(SshTunnelStream { stream, session })
}

/// Local TCP listener on `127.0.0.1` with an ephemeral port, forwarded through the bastion.
pub async fn open_local_forward(params: &SshConnectParams) -> Result<(u16, SshSession), SshError> {
    let handle = establish_ssh_session(params).await?;
    let shared = Arc::new(Mutex::new(handle));
    let session = SshSession {
        _handle: Arc::clone(&shared),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr()?.port();
    let target_host = params.target_host.clone();
    let target_port = params.target_port;
    tokio::spawn(async move {
        loop {
            let Ok((mut tcp, _)) = listener.accept().await else {
                break;
            };
            let h = Arc::clone(&shared);
            let th = target_host.clone();
            tokio::spawn(async move {
                let Ok(channel) = h
                    .lock()
                    .await
                    .channel_open_direct_tcpip(&th, u32::from(target_port), "127.0.0.1", 0)
                    .await
                else {
                    return;
                };
                let mut ch_stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut tcp, &mut ch_stream).await;
            });
        }
    });
    Ok((local_port, session))
}

async fn establish_ssh_session(
    params: &SshConnectParams,
) -> Result<client::Handle<SshClientHandler>, SshError> {
    let (host, port, user, identity) = resolve_bastion(params)?;
    let addr: SocketAddr = format!("{host}:{port}")
        .parse::<SocketAddr>()
        .map_err(|e: std::net::AddrParseError| SshError::Connect(e.to_string()))?;

    let known_hosts = KnownHosts::wisp_default().map_err(SshError::Io)?;
    let handler = SshClientHandler {
        bastion_host: host.clone(),
        bastion_port: port,
        known_hosts: Arc::new(std::sync::Mutex::new(known_hosts)),
        allow_tofu: params.allow_tofu,
    };

    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(300)),
        keepalive_interval: Some(Duration::from_secs(30)),
        ..client::Config::default()
    });

    let mut handle = client::connect(config, addr, handler).await?;
    authenticate(
        &mut handle,
        &user,
        &params.settings,
        &params.secrets,
        identity.as_deref(),
    )
    .await?;
    Ok(handle)
}

fn resolve_bastion(
    params: &SshConnectParams,
) -> Result<(String, u16, String, Option<std::path::PathBuf>), SshError> {
    let mut host = params
        .settings
        .host
        .clone()
        .ok_or_else(|| SshError::Connect("SSH bastion host is required".into()))?;
    let mut port = params.settings.port.unwrap_or(22);
    let mut user = params
        .settings
        .user
        .clone()
        .unwrap_or_else(|| "root".into());
    let mut identity = params
        .settings
        .identity_file
        .as_ref()
        .map(std::path::PathBuf::from);

    if let Some(alias) = params.settings.config_host.as_deref() {
        let cfg = OpenSshConfig::load_default().unwrap_or_default();
        let block = cfg.resolve(alias);
        if let Some(h) = block.host_name {
            host = h;
        }
        if let Some(p) = block.port {
            port = p;
        }
        if let Some(u) = block.user {
            user = u;
        }
        if identity.is_none() {
            identity = block.identity_file;
        }
    }

    Ok((host, port, user, identity))
}

async fn authenticate(
    handle: &mut client::Handle<SshClientHandler>,
    user: &str,
    settings: &SshSettings,
    secrets: &SshSecrets,
    identity: Option<&std::path::Path>,
) -> Result<(), SshError> {
    let auth = match settings.auth {
        SshAuthMethod::Password => {
            let password = secrets.password.as_deref().ok_or(SshError::AuthFailed)?;
            handle
                .authenticate_password(user, password)
                .await
                .map_err(|_| SshError::AuthFailed)?
        }
        SshAuthMethod::PublicKey => {
            let path = identity.ok_or(SshError::AuthFailed)?;
            let key = load_secret_key(path, secrets.key_passphrase.as_deref())
                .map_err(|e| SshError::Connect(e.to_string()))?;
            let rsa_hash = handle
                .best_supported_rsa_hash()
                .await
                .ok()
                .flatten()
                .flatten();
            handle
                .authenticate_publickey(
                    user,
                    PrivateKeyWithHashAlg::new(Arc::new(key), rsa_hash),
                )
                .await
                .map_err(|_| SshError::AuthFailed)?
        }
        SshAuthMethod::Agent => {
            if !settings.use_agent {
                return Err(SshError::AuthFailed);
            }
            agent_auth(handle, user).await?
        }
    };
    if auth.success() {
        Ok(())
    } else {
        Err(SshError::AuthFailed)
    }
}

async fn agent_auth(
    handle: &mut client::Handle<SshClientHandler>,
    user: &str,
) -> Result<AuthResult, SshError> {
    let stream = connect_agent().await?;
    let mut agent = agent::client::AgentClient::connect(stream);
    let keys = agent
        .request_identities()
        .await
        .map_err(|e| SshError::Connect(e.to_string()))?;
    let Some(identity) = keys.into_iter().next() else {
        return Err(SshError::AuthFailed);
    };
    let key = agent_public_key(identity);
    handle
        .authenticate_publickey_with(user, key, None, &mut agent)
        .await
        .map_err(|_| SshError::AuthFailed)
}

fn agent_public_key(identity: AgentIdentity) -> PublicKey {
    identity.public_key().into_owned()
}

async fn connect_agent() -> Result<tokio::net::UnixStream, SshError> {
    let sock = std::env::var("SSH_AUTH_SOCK").map_err(|_| SshError::AuthFailed)?;
    tokio::net::UnixStream::connect(sock)
        .await
        .map_err(|_| SshError::AuthFailed)
}

struct SshClientHandler {
    bastion_host: String,
    bastion_port: u16,
    known_hosts: Arc<std::sync::Mutex<KnownHosts>>,
    allow_tofu: bool,
}

impl Handler for SshClientHandler {
    type Error = SshError;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = server_public_key.public_key();
        let mut store = self
            .known_hosts
            .lock()
            .map_err(|_| SshError::Connect("known_hosts lock poisoned".into()))?;
        store.verify_or_trust(
            &self.bastion_host,
            self.bastion_port,
            &key,
            self.allow_tofu,
        )?;
        Ok(true)
    }
}
