use thiserror::Error;

#[derive(Debug, Error)]
pub enum SshError {
    #[error("SSH connection failed: {0}")]
    Connect(String),
    #[error("SSH authentication failed")]
    AuthFailed,
    #[error("SSH host key changed — fingerprint was {expected}, server presented {actual}")]
    HostKeyChanged {
        expected: String,
        actual: String,
    },
    #[error("SSH host key unknown — fingerprint {fingerprint} (trust on first use not enabled)")]
    UnknownHostKey { fingerprint: String },
    #[error("SSH tunnel failed: {0}")]
    Tunnel(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Russh(#[from] russh::Error),
}
