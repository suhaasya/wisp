//! tokio-postgres rustls connector.

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_postgres::tls::{ChannelBinding, MakeTlsConnect, TlsConnect, TlsStream};
use tokio_rustls::client::TlsStream as TokioRustlsStream;
use tokio_rustls::TlsConnector;

use super::TlsNegotiated;

#[derive(Clone)]
pub struct PostgresTlsMaker {
    connector: TlsConnector,
    last_negotiated: Arc<Mutex<Option<TlsNegotiated>>>,
}

impl PostgresTlsMaker {
    pub fn new(config: Arc<rustls::ClientConfig>) -> Self {
        Self {
            connector: TlsConnector::from(config),
            last_negotiated: Arc::new(Mutex::new(None)),
        }
    }

    pub fn take_negotiated(&self) -> Option<TlsNegotiated> {
        self.last_negotiated.lock().ok()?.take()
    }
}

pub struct PostgresTlsConnector<S> {
    maker: PostgresTlsMaker,
    domain: String,
    _stream: std::marker::PhantomData<S>,
}

impl<S> MakeTlsConnect<S> for PostgresTlsMaker
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Stream = PostgresTlsStream<S>;
    type TlsConnect = PostgresTlsConnector<S>;
    type Error = std::io::Error;

    fn make_tls_connect(&mut self, domain: &str) -> Result<Self::TlsConnect, Self::Error> {
        Ok(PostgresTlsConnector {
            maker: self.clone(),
            domain: domain.to_string(),
            _stream: std::marker::PhantomData,
        })
    }
}

impl<S> TlsConnect<S> for PostgresTlsConnector<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Stream = PostgresTlsStream<S>;
    type Error = std::io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Stream, Self::Error>> + Send>>;

    fn connect(self, stream: S) -> Self::Future {
        let domain = self.domain;
        let connector = self.maker.connector.clone();
        let negotiated = Arc::clone(&self.maker.last_negotiated);
        Box::pin(async move {
            let server_name = ServerName::try_from(domain)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
            let tls = connector
                .connect(server_name, stream)
                .await
                .map_err(std::io::Error::other)?;
            if let Ok(mut guard) = negotiated.lock() {
                *guard = Some(TlsNegotiated::from_rustls(tls.get_ref().1));
            }
            Ok(PostgresTlsStream(tls))
        })
    }
}

pub struct PostgresTlsStream<S>(TokioRustlsStream<S>);

impl<S> TlsStream for PostgresTlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    fn channel_binding(&self) -> ChannelBinding {
        ChannelBinding::none()
    }
}

impl<S> AsyncRead for PostgresTlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for PostgresTlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}
