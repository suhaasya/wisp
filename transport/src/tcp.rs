//! Plain TCP streams (TLS wraps this type in LUM-018).

use std::{future::Future, pin::Pin};

use tokio::net::TcpStream;

/// Async byte stream used for database wire protocols before TLS upgrade.
pub trait AsyncDbStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static {}

impl AsyncDbStream for TcpStream {}

/// Type-erased stream for connectors that may return TCP or TLS later.
pub type BoxDbStream = Pin<Box<dyn AsyncDbStream>>;

pub async fn connect_tcp(host: &str, port: u16) -> std::io::Result<TcpStream> {
    let addrs = format!("{host}:{port}");
    TcpStream::connect(addrs).await
}

/// Connect helper accepting a custom connector (tests, future TLS).
pub async fn connect_with<F, S, E>(host: &str, port: u16, connect: F) -> Result<S, E>
where
    F: FnOnce(&str, u16) -> Pin<Box<dyn Future<Output = Result<S, E>> + Send>>,
{
    connect(host, port).await
}
