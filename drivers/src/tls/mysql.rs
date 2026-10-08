//! Map [`wisp_store::SslSettings`] to `mysql_async::SslOpts`.

use std::path::PathBuf;

use mysql_async::{ClientIdentity, SslOpts};
use wisp_store::{SslMode, SslSettings, SslTrustStore};

pub fn mysql_ssl_opts(ssl: &SslSettings) -> Option<SslOpts> {
    if ssl.mode == SslMode::Disable {
        return None;
    }

    let (accept_invalid, skip_domain) = match ssl.mode {
        SslMode::Disable => return None,
        SslMode::Prefer | SslMode::Require => (true, true),
        SslMode::VerifyCa => (false, true),
        SslMode::VerifyFull => (false, false),
    };

    let mut opts = SslOpts::default()
        .with_danger_accept_invalid_certs(accept_invalid)
        .with_danger_skip_domain_validation(skip_domain);

    if ssl.trust == SslTrustStore::CustomCa {
        opts = opts.with_disable_built_in_roots(true);
    }

    if let Some(path) = ssl.ca_file.as_deref().filter(|p| !p.is_empty()) {
        opts = opts.with_root_certs(vec![PathBuf::from(path).into()]);
    }

    if let (Some(cert), Some(key)) = (
        ssl.client_cert_file.clone().filter(|p| !p.is_empty()),
        ssl.client_key_file.clone().filter(|p| !p.is_empty()),
    ) {
        opts = opts.with_client_identity(Some(ClientIdentity::new(
            PathBuf::from(cert).into(),
            PathBuf::from(key).into(),
        )));
    }

    Some(opts)
}
