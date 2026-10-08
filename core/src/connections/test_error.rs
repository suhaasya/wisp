//! User-facing connection test errors (LUM-016).

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConnectionTestError {
    #[error("DNS lookup failed — check the hostname.")]
    DnsFailure,
    #[error("Connection refused — is the server running and is the port correct?")]
    ConnectionRefused,
    #[error("Authentication failed — check username and password.")]
    AuthFailed,
    #[error("TLS mismatch — server requires a different SSL/TLS mode (see SSL tab).")]
    TlsMismatch,
    #[error("Hostname does not match the server certificate — check the host or use verify-ca.")]
    HostnameMismatch,
    #[error("SSH host key changed — verify the bastion fingerprint before reconnecting.")]
    SshHostKeyChanged,
    #[error("SSH host key unknown — confirm the bastion fingerprint to trust it.")]
    SshHostKeyUnknown,
    #[error("Connection timed out after 10 seconds.")]
    Timeout,
    #[error("The test was cancelled.")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

impl ConnectionTestError {
    pub fn classify(message: &str, detail: &str) -> Self {
        let haystack = format!("{message} {detail}").to_lowercase();
        if haystack.contains("timed out") || haystack.contains("timeout") {
            return Self::Timeout;
        }
        if haystack.contains("failed to lookup")
            || haystack.contains("name or service not known")
            || haystack.contains("nodename nor servname")
            || haystack.contains("dns")
        {
            return Self::DnsFailure;
        }
        if haystack.contains("connection refused") || haystack.contains("actively refused") {
            return Self::ConnectionRefused;
        }
        if haystack.contains("host key changed") {
            return Self::SshHostKeyChanged;
        }
        if haystack.contains("host key unknown") {
            return Self::SshHostKeyUnknown;
        }
        if haystack.contains("ssh authentication failed") {
            return Self::AuthFailed;
        }
        if haystack.contains("password authentication failed")
            || haystack.contains("authentication failed")
            || haystack.contains("access denied")
            || haystack.contains("auth")
                && (haystack.contains("failed") || haystack.contains("denied"))
        {
            return Self::AuthFailed;
        }
        if haystack.contains("notvalidforname")
            || haystack.contains("certificate not valid for name")
            || (haystack.contains("invalid certificate") && haystack.contains("name"))
        {
            return Self::HostnameMismatch;
        }
        if haystack.contains("ssl")
            || haystack.contains("tls")
            || haystack.contains("certificate")
            || haystack.contains("encrypt")
        {
            return Self::TlsMismatch;
        }
        Self::Other(message.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_dns() {
        assert_eq!(
            ConnectionTestError::classify("TCP connect failed", "failed to lookup address info"),
            ConnectionTestError::DnsFailure
        );
    }

    #[test]
    fn maps_refused() {
        assert_eq!(
            ConnectionTestError::classify("TCP connect failed", "Connection refused (os error 111)"),
            ConnectionTestError::ConnectionRefused
        );
    }

    #[test]
    fn maps_postgres_auth() {
        assert_eq!(
            ConnectionTestError::classify(
                "Could not connect to PostgreSQL",
                "password authentication failed for user \"app\""
            ),
            ConnectionTestError::AuthFailed
        );
    }

    #[test]
    fn maps_mysql_auth() {
        assert_eq!(
            ConnectionTestError::classify("MySQL error", "Access denied for user 'root'@'localhost'"),
            ConnectionTestError::AuthFailed
        );
    }

    #[test]
    fn maps_tls() {
        assert_eq!(
            ConnectionTestError::classify("Could not connect", "SSL connection required"),
            ConnectionTestError::TlsMismatch
        );
    }

    #[test]
    fn maps_hostname_mismatch() {
        assert_eq!(
            ConnectionTestError::classify(
                "Could not connect to PostgreSQL",
                "certificate not valid for name"
            ),
            ConnectionTestError::HostnameMismatch
        );
    }

    #[test]
    fn maps_timeout() {
        assert_eq!(
            ConnectionTestError::classify("TCP connect failed", "operation timed out"),
            ConnectionTestError::Timeout
        );
    }
}
