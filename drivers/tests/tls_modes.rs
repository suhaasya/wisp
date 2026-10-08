//! TLS mode mapping tests (LUM-018).

use wisp_drivers::mysql_ssl_opts;
use wisp_store::{SslMode, SslSettings, SslTrustStore};

#[test]
fn mysql_disable_has_no_ssl_opts() {
    let ssl = SslSettings {
        mode: SslMode::Disable,
        ..SslSettings::default()
    };
    assert!(mysql_ssl_opts(&ssl).is_none());
}

#[test]
fn mysql_require_allows_invalid_certs() {
    let ssl = SslSettings {
        mode: SslMode::Require,
        ..SslSettings::default()
    };
    let opts = mysql_ssl_opts(&ssl).expect("ssl opts");
    assert!(opts.accept_invalid_certs());
    assert!(opts.skip_domain_validation());
}

#[test]
fn mysql_verify_full_is_strict() {
    let ssl = SslSettings {
        mode: SslMode::VerifyFull,
        ..SslSettings::default()
    };
    let opts = mysql_ssl_opts(&ssl).expect("ssl opts");
    assert!(!opts.accept_invalid_certs());
    assert!(!opts.skip_domain_validation());
}

#[test]
fn mysql_custom_ca_disables_builtin_roots() {
    let ssl = SslSettings {
        mode: SslMode::Require,
        trust: SslTrustStore::CustomCa,
        ca_file: Some("/tmp/ca.pem".into()),
        ..SslSettings::default()
    };
    let opts = mysql_ssl_opts(&ssl).expect("ssl opts");
    assert!(opts.disable_built_in_roots());
    assert_eq!(opts.root_certs().len(), 1);
}
