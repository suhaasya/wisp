//! Connection URL / libpq / `.env` paste parsing (LUM-017).

use std::collections::BTreeMap;

use percent_encoding::percent_decode_str;
use thiserror::Error;
use url::Url;
use wisp_store::SslMode;

use super::form::{ConnectionFormDraft, FormEngine, FormEnvironment};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UrlParseError {
    #[error("not a recognized connection string")]
    Unrecognized,
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
}

/// Parse pasted text into a form draft. Password material is never logged ([`fmt::Debug`] redacts it).
pub fn parse_connection_paste(input: &str) -> Result<ConnectionFormDraft, UrlParseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(UrlParseError::Unrecognized);
    }

    let payload = extract_env_value(trimmed).unwrap_or(trimmed);

    if looks_like_url(payload) {
        return parse_url(payload);
    }
    if looks_like_libpq(payload) {
        return parse_libpq(payload);
    }
    Err(UrlParseError::Unrecognized)
}

fn looks_like_url(s: &str) -> bool {
    matches!(
        s.split(':').next(),
        Some("postgres") | Some("postgresql") | Some("mysql") | Some("mariadb")
    )
}

fn looks_like_libpq(s: &str) -> bool {
    s.contains('=') && !s.contains("://")
}

fn extract_env_value(line: &str) -> Option<&str> {
    let line = line.trim();
    let line = line.strip_prefix("export ").unwrap_or(line);
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    if key.eq_ignore_ascii_case("DATABASE_URL")
        || key.ends_with("_DATABASE_URL")
        || key.eq_ignore_ascii_case("DB_URL")
    {
        Some(unquote_value(value.trim()))
    } else {
        None
    }
}

fn unquote_value(value: &str) -> &str {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        return &value[1..value.len() - 1];
    }
    value
}

fn parse_url(raw: &str) -> Result<ConnectionFormDraft, UrlParseError> {
    let url = Url::parse(raw).map_err(|e| UrlParseError::InvalidUrl(e.to_string()))?;
    let engine = match url.scheme() {
        "postgres" | "postgresql" => FormEngine::PostgreSql,
        "mysql" => FormEngine::MySql,
        "mariadb" => FormEngine::MariaDb,
        _ => return Err(UrlParseError::Unrecognized),
    };

    let host = url
        .host_str()
        .ok_or(UrlParseError::MissingField("host"))?
        .to_string();
    let port = url.port().unwrap_or_else(|| engine.default_port());
    let user = decode_component(url.username());
    let password = url
        .password()
        .map(decode_component)
        .unwrap_or_default();
    let database = url
        .path()
        .trim_start_matches('/')
        .split('/')
        .next()
        .filter(|s| !s.is_empty())
        .map_or_else(|| default_database(engine), str::to_string);

    let mut extra_options = BTreeMap::new();
    let mut ssl_mode = None;
    for (key, value) in url.query_pairs() {
        let key_str = key.as_ref();
        let value_str = value.as_ref();
        if let Some(mode) = map_ssl_param(engine, key_str, value_str) {
            ssl_mode = Some(mode);
        } else if !map_known_option(engine, key_str, value_str, &mut extra_options) {
            extra_options.insert(key_str.to_string(), value_str.to_string());
        }
    }

    let ssl_mode = ssl_mode
        .or_else(|| provider_tls_default(&host))
        .unwrap_or(SslMode::Disable);

    Ok(build_draft(
        engine,
        host,
        port,
        user,
        password,
        database,
        ssl_mode,
        extra_options,
    ))
}

fn parse_libpq(raw: &str) -> Result<ConnectionFormDraft, UrlParseError> {
    let pairs = tokenize_libpq_pairs(raw)?;
    let engine = FormEngine::PostgreSql;
    let host = pairs
        .get("host")
        .cloned()
        .ok_or(UrlParseError::MissingField("host"))?;
    let port = pairs
        .get("port")
        .and_then(|p| p.parse().ok())
        .unwrap_or_else(|| engine.default_port());
    let user = pairs.get("user").cloned().unwrap_or_else(|| "postgres".into());
    let password = pairs.get("password").cloned().unwrap_or_default();
    let database = pairs
        .get("dbname")
        .or_else(|| pairs.get("database"))
        .cloned()
        .unwrap_or_else(|| default_database(engine));

    let mut extra_options = BTreeMap::new();
    let mut ssl_mode = None;
    for (key, value) in &pairs {
        match key.as_str() {
            "host" | "port" | "user" | "password" | "dbname" | "database" => {}
            "sslmode" => ssl_mode = map_postgres_sslmode(value),
            "application_name" => {
                extra_options.insert("application_name".into(), value.clone());
            }
            "options" => {
                extra_options.insert("options".into(), value.clone());
            }
            _ => {
                extra_options.insert(key.clone(), value.clone());
            }
        }
    }
    let ssl_mode = ssl_mode
        .or_else(|| provider_tls_default(&host))
        .unwrap_or(SslMode::Disable);

    Ok(build_draft(
        engine,
        host,
        port,
        user,
        password,
        database,
        ssl_mode,
        extra_options,
    ))
}

fn tokenize_libpq_pairs(raw: &str) -> Result<BTreeMap<String, String>, UrlParseError> {
    let mut out = BTreeMap::new();
    for token in tokenize_libpq(raw) {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        out.insert(key.trim().to_lowercase(), unquote_libpq_value(value.trim()));
    }
    if out.is_empty() {
        return Err(UrlParseError::Unrecognized);
    }
    Ok(out)
}

fn tokenize_libpq(raw: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;
    for ch in raw.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }
        match ch {
            '\\' if in_single || in_double => escape = true,
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            ' ' | '\t' if !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn unquote_libpq_value(value: &str) -> String {
    if value.len() >= 2
        && ((value.starts_with('\'') && value.ends_with('\''))
            || (value.starts_with('"') && value.ends_with('"')))
    {
        return value[1..value.len() - 1].replace("\\'", "'").replace("\\\"", "\"");
    }
    value.to_string()
}

#[allow(clippy::too_many_arguments)]
fn build_draft(
    engine: FormEngine,
    host: String,
    port: u16,
    user: String,
    password: String,
    database: String,
    ssl_mode: SslMode,
    extra_options: BTreeMap<String, String>,
) -> ConnectionFormDraft {
    let name = if database.is_empty() {
        format!("{user}@{host}")
    } else {
        database.clone()
    };
    ConnectionFormDraft {
        id: None,
        engine,
        name,
        folder_id: None,
        host,
        port: port.to_string(),
        user,
        password,
        database,
        save_password: true,
        environment: FormEnvironment::Dev,
        read_only: false,
        safe_mode: false,
        ssl_mode,
        ssl_trust: wisp_store::SslTrustStore::System,
        ssl_ca_file: String::new(),
        ssl_client_cert_file: String::new(),
        ssl_client_key_file: String::new(),
        ssh_enabled: false,
        ssh_host: String::new(),
        ssh_port: "22".into(),
        ssh_user: String::new(),
        ssh_auth: wisp_store::SshAuthMethod::Agent,
        ssh_identity_file: String::new(),
        ssh_use_agent: true,
        ssh_config_host: String::new(),
        ssh_password: String::new(),
        ssh_key_passphrase: String::new(),
        extra_options,
    }
}

fn default_database(engine: FormEngine) -> String {
    match engine {
        FormEngine::PostgreSql => "postgres".into(),
        FormEngine::MySql | FormEngine::MariaDb => "mysql".into(),
    }
}

fn decode_component(s: &str) -> String {
    percent_decode_str(s).decode_utf8_lossy().into_owned()
}

fn map_ssl_param(engine: FormEngine, key: &str, value: &str) -> Option<SslMode> {
    match engine {
        FormEngine::PostgreSql => {
            if key.eq_ignore_ascii_case("sslmode") {
                map_postgres_sslmode(value)
            } else {
                None
            }
        }
        FormEngine::MySql | FormEngine::MariaDb => {
            if key.eq_ignore_ascii_case("ssl-mode") || key.eq_ignore_ascii_case("sslmode") {
                map_mysql_ssl_mode(value)
            } else {
                None
            }
        }
    }
}

fn map_known_option(
    engine: FormEngine,
    key: &str,
    value: &str,
    options: &mut BTreeMap<String, String>,
) -> bool {
    match engine {
        FormEngine::PostgreSql => {
            if key.eq_ignore_ascii_case("application_name") {
                options.insert("application_name".into(), value.into());
                true
            } else if key.eq_ignore_ascii_case("options") {
                options.insert("options".into(), value.into());
                true
            } else {
                false
            }
        }
        FormEngine::MySql | FormEngine::MariaDb => {
            if key.eq_ignore_ascii_case("charset") {
                options.insert("charset".into(), value.into());
                true
            } else {
                false
            }
        }
    }
}

fn map_postgres_sslmode(value: &str) -> Option<SslMode> {
    match value.to_ascii_lowercase().as_str() {
        "disable" => Some(SslMode::Disable),
        "prefer" => Some(SslMode::Prefer),
        "require" => Some(SslMode::Require),
        "verify-ca" => Some(SslMode::VerifyCa),
        "verify-full" => Some(SslMode::VerifyFull),
        _ => None,
    }
}

fn map_mysql_ssl_mode(value: &str) -> Option<SslMode> {
    match value.to_ascii_uppercase().as_str() {
        "DISABLED" | "DISABLE" => Some(SslMode::Disable),
        "PREFERRED" | "PREFER" => Some(SslMode::Prefer),
        "REQUIRED" | "REQUIRE" => Some(SslMode::Require),
        "VERIFY_CA" | "VERIFY-CA" => Some(SslMode::VerifyCa),
        "VERIFY_IDENTITY" | "VERIFY-FULL" => Some(SslMode::VerifyFull),
        _ => None,
    }
}

/// Hosted-provider defaults when the URL omits explicit TLS mode.
pub fn provider_tls_default(host: &str) -> Option<SslMode> {
    let host = host.to_ascii_lowercase();
    if host.contains("supabase.co")
        || host.contains("neon.tech")
        || host.contains("rds.amazonaws.com")
        || host.contains("psdb.cloud")
        || host.contains("planetscale.com")
        || host.contains("sql.cloud.google.com")
        || host.contains("cloudsql")
    {
        Some(SslMode::Require)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> ConnectionFormDraft {
        parse_connection_paste(input).expect("parse")
    }

    #[test]
    fn postgres_basic() {
        let d = parse("postgres://app:secret@db.example.com:5432/shop");
        assert_eq!(d.engine, FormEngine::PostgreSql);
        assert_eq!(d.host, "db.example.com");
        assert_eq!(d.port, "5432");
        assert_eq!(d.user, "app");
        assert_eq!(d.password, "secret");
        assert_eq!(d.database, "shop");
    }

    #[test]
    fn postgresql_scheme() {
        let d = parse("postgresql://u@localhost/mydb");
        assert_eq!(d.engine, FormEngine::PostgreSql);
        assert_eq!(d.port, "5432");
        assert_eq!(d.database, "mydb");
    }

    #[test]
    fn percent_encoded_password() {
        let d = parse("postgres://u:p%40ss%2Fword@h/db");
        assert_eq!(d.password, "p@ss/word");
    }

    #[test]
    fn sslmode_require() {
        let d = parse("postgres://u@h/db?sslmode=require");
        assert_eq!(d.ssl_mode, SslMode::Require);
    }

    #[test]
    fn application_name_and_unknown_query() {
        let d = parse("postgres://u@h/db?application_name=wisp&foo=bar");
        assert_eq!(d.extra_options.get("application_name").map(String::as_str), Some("wisp"));
        assert_eq!(d.extra_options.get("foo").map(String::as_str), Some("bar"));
    }

    #[test]
    fn mysql_url() {
        let d = parse("mysql://root:pw@127.0.0.1:3307/app?ssl-mode=REQUIRED&charset=utf8mb4");
        assert_eq!(d.engine, FormEngine::MySql);
        assert_eq!(d.port, "3307");
        assert_eq!(d.ssl_mode, SslMode::Require);
        assert_eq!(d.extra_options.get("charset").map(String::as_str), Some("utf8mb4"));
    }

    #[test]
    fn mariadb_url() {
        let d = parse("mariadb://u@host/db");
        assert_eq!(d.engine, FormEngine::MariaDb);
    }

    #[test]
    fn libpq_string() {
        let d = parse("host=localhost port=5433 dbname=shop user=app password=x sslmode=verify-full");
        assert_eq!(d.host, "localhost");
        assert_eq!(d.port, "5433");
        assert_eq!(d.ssl_mode, SslMode::VerifyFull);
    }

    #[test]
    fn env_line_double_quoted() {
        let d = parse("DATABASE_URL=\"postgres://u:pw@h/db\"");
        assert_eq!(d.user, "u");
        assert_eq!(d.password, "pw");
    }

    #[test]
    fn env_export_prefix() {
        let d = parse("export DATABASE_URL=postgres://u@h/db");
        assert_eq!(d.database, "db");
    }

    #[test]
    fn neon_provider_default_tls() {
        let d = parse("postgres://u@ep-cool-name.neon.tech/neondb");
        assert_eq!(d.ssl_mode, SslMode::Require);
    }

    #[test]
    fn debug_draft_never_prints_password() {
        let d = parse("postgres://u:topsecret@h/db");
        let debug = format!("{d:?}");
        assert!(!debug.contains("topsecret"));
    }
}
