//! Low-level connection transports (TLS, SSH tunnels, plain TCP).
//!
//! Owns wire-level connection setup and secure channel primitives. Must not depend on
//! database drivers, session logic, UI, or persistent storage.

pub mod ssh;
pub mod tcp;

pub use ssh::{
    fingerprint_sha256, open_direct_tcpip, open_local_forward, OpenSshConfig, SshConnectParams,
    SshError, SshSecrets, SshSession, SshTunnelStream,
};
pub use tcp::{connect_tcp, connect_with, AsyncDbStream, BoxDbStream};
pub use tokio::net::TcpStream as PlainStream;

pub const CRATE_MARKER: &str = "wisp-transport";

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(super::CRATE_MARKER, "wisp-transport");
    }
}
