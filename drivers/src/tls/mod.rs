//! rustls TLS for PostgreSQL and MySQL (LUM-018).

mod config;
mod mysql;
mod postgres;

pub use config::{build_rustls_config, is_local_host, TlsNegotiated, TlsSetupError};
pub use mysql::mysql_ssl_opts;
pub use postgres::PostgresTlsMaker;
