//! Shared rustls client configuration from [`wisp_store::SslSettings`].

use std::{fs, path::Path, sync::Arc};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error, RootCertStore, SignatureScheme};
use rustls_pemfile::{certs, pkcs8_private_keys, rsa_private_keys};
use rustls_platform_verifier::BuilderVerifierExt;
use thiserror::Error;
use wisp_store::{SslMode, SslSettings, SslTrustStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsNegotiated {
    pub version: String,
    pub cipher: String,
}

impl TlsNegotiated {
    pub fn summary(&self) -> String {
        format!("{} / {}", self.version, self.cipher)
    }

    pub fn from_rustls(conn: &rustls::ClientConnection) -> Self {
        let version = conn
            .protocol_version()
            .map_or_else(|| "unknown".into(), |v| format!("{v:?}"));
        let cipher = conn
            .negotiated_cipher_suite()
            .map_or_else(|| "unknown".into(), |c| format!("{:?}", c.suite()));
        Self { version, cipher }
    }
}

#[derive(Debug, Error)]
pub enum TlsSetupError {
    #[error("read {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("TLS configuration: {0}")]
    Config(String),
    #[error("custom CA file is required when trust is set to custom CA")]
    MissingCaFile,
}

pub fn is_local_host(host: &str) -> bool {
    let host = host.trim().trim_matches(['[', ']']);
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if host == "127.0.0.1" || host == "::1" {
        return true;
    }
    if host.ends_with(".local") {
        return true;
    }
    false
}

pub fn build_rustls_config(
    ssl: &SslSettings,
    _server_host: &str,
) -> Result<Option<Arc<ClientConfig>>, TlsSetupError> {
    if ssl.mode == SslMode::Disable {
        return Ok(None);
    }

    let client_auth = load_client_identity(ssl)?;

    if ssl.mode == SslMode::VerifyFull && ssl.trust == SslTrustStore::System {
        let builder = ClientConfig::builder().with_platform_verifier();
        let config = if let Some((chain, key)) = client_auth {
            builder
                .with_client_auth_cert(chain, key)
                .map_err(|e| TlsSetupError::Config(e.to_string()))?
        } else {
            builder.with_no_client_auth()
        };
        return Ok(Some(Arc::new(config)));
    }

    let roots = load_roots(ssl)?;
    let builder = ClientConfig::builder().with_root_certificates(roots.clone());
    let mut config = if let Some((chain, key)) = client_auth {
        builder
            .with_client_auth_cert(chain, key)
            .map_err(|e| TlsSetupError::Config(e.to_string()))?
    } else {
        builder.with_no_client_auth()
    };

    match ssl.mode {
        SslMode::Disable => {}
        SslMode::Prefer | SslMode::Require => {
            config
                .dangerous()
                .set_certificate_verifier(Arc::new(AcceptAnyVerifier));
        }
        SslMode::VerifyCa => {
            let inner = WebPkiServerVerifier::builder(Arc::new(roots))
                .build()
                .map_err(|e| TlsSetupError::Config(e.to_string()))?;
            config.dangerous().set_certificate_verifier(Arc::new(
                FlexibleVerifier {
                    inner,
                    skip_hostname: true,
                },
            ));
        }
        SslMode::VerifyFull => {
            let inner = WebPkiServerVerifier::builder(Arc::new(roots))
                .build()
                .map_err(|e| TlsSetupError::Config(e.to_string()))?;
            config
                .dangerous()
                .set_certificate_verifier(Arc::new(FlexibleVerifier {
                    inner,
                    skip_hostname: false,
                }));
        }
    }

    Ok(Some(Arc::new(config)))
}

fn load_roots(ssl: &SslSettings) -> Result<RootCertStore, TlsSetupError> {
    let mut roots = RootCertStore::empty();
    if ssl.trust == SslTrustStore::System {
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }
    if let Some(path) = ssl.ca_file.as_deref().filter(|p| !p.is_empty()) {
        add_ca_file(&mut roots, path)?;
    } else if ssl.trust == SslTrustStore::CustomCa {
        return Err(TlsSetupError::MissingCaFile);
    }
    Ok(roots)
}

#[derive(Debug)]
struct AcceptAnyVerifier;

impl ServerCertVerifier for AcceptAnyVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[derive(Debug)]
struct FlexibleVerifier {
    inner: Arc<WebPkiServerVerifier>,
    skip_hostname: bool,
}

impl ServerCertVerifier for FlexibleVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        match self
            .inner
            .verify_server_cert(end_entity, intermediates, server_name, ocsp, now)
        {
            Ok(v) => Ok(v),
            Err(e) if self.skip_hostname && is_name_mismatch(&e) => Ok(ServerCertVerified::assertion()),
            Err(e) => Err(e),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

fn is_name_mismatch(err: &Error) -> bool {
    let text = err.to_string().to_lowercase();
    text.contains("notvalidforname")
        || text.contains("certificate not valid for name")
        || text.contains("invalid name")
        || text.contains("hostname")
}

fn add_ca_file(store: &mut RootCertStore, path: &str) -> Result<(), TlsSetupError> {
    let pem = fs::read(path).map_err(|source| TlsSetupError::Io {
        path: path.to_string(),
        source,
    })?;
    let mut reader = pem.as_slice();
    for cert in certs(&mut reader) {
        let cert = cert.map_err(|e| TlsSetupError::Config(e.to_string()))?;
        store
            .add(cert)
            .map_err(|e| TlsSetupError::Config(e.to_string()))?;
    }
    Ok(())
}

fn load_client_identity(
    ssl: &SslSettings,
) -> Result<Option<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)>, TlsSetupError> {
    let Some(cert_path) = ssl.client_cert_file.as_deref().filter(|p| !p.is_empty()) else {
        return Ok(None);
    };
    let key_path = ssl
        .client_key_file
        .as_deref()
        .filter(|p| !p.is_empty())
        .ok_or_else(|| TlsSetupError::Config("client key file is required with client cert".into()))?;

    let chain = load_cert_chain(cert_path)?;
    let key = load_private_key(key_path)?;
    Ok(Some((chain, key)))
}

fn load_cert_chain(path: &str) -> Result<Vec<CertificateDer<'static>>, TlsSetupError> {
    let pem = fs::read(path).map_err(|source| TlsSetupError::Io {
        path: path.to_string(),
        source,
    })?;
    let mut reader = pem.as_slice();
    certs(&mut reader)
        .map(|c| c.map_err(|e| TlsSetupError::Config(e.to_string())))
        .collect()
}

fn load_private_key(path: &str) -> Result<PrivateKeyDer<'static>, TlsSetupError> {
    let pem = fs::read(path).map_err(|source| TlsSetupError::Io {
        path: path.to_string(),
        source,
    })?;
    let mut reader = pem.as_slice();
    if let Some(key) = pkcs8_private_keys(&mut reader)
        .next()
        .transpose()
        .map_err(|e| TlsSetupError::Config(e.to_string()))?
    {
        return Ok(PrivateKeyDer::Pkcs8(key));
    }
    let mut reader = pem.as_slice();
    if let Some(key) = rsa_private_keys(&mut reader)
        .next()
        .transpose()
        .map_err(|e| TlsSetupError::Config(e.to_string()))?
    {
        return Ok(PrivateKeyDer::Pkcs1(key));
    }
    Err(TlsSetupError::Config(format!(
        "no PKCS#8 or RSA private key found in {}",
        Path::new(path).display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_host_detection() {
        assert!(is_local_host("localhost"));
        assert!(is_local_host("127.0.0.1"));
        assert!(is_local_host("db.local"));
        assert!(!is_local_host("db.example.com"));
    }
}
