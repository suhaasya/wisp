//! `.pgpass` file lookup (PostgreSQL password file).

use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use crate::error::DriverError;

/// Resolve password: explicit config wins, then `.pgpass`.
pub fn resolve_password(
    host: &str,
    port: u16,
    database: &str,
    user: &str,
    explicit: Option<&str>,
) -> Result<Option<String>, DriverError> {
    if let Some(password) = explicit {
        return Ok(Some(password.to_owned()));
    }
    lookup_pgpass(host, port, database, user)
}

fn lookup_pgpass(
    host: &str,
    port: u16,
    database: &str,
    user: &str,
) -> Result<Option<String>, DriverError> {
    let path = pgpass_path()?;
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(DriverError::user(
                "Could not read password file",
                err.to_string(),
            ))
        }
    };
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line.map_err(|e| DriverError::user("Could not read password file", e.to_string()))?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(entry) = parse_line(line) else {
            continue;
        };
        if entry.matches(host, port, database, user) {
            return Ok(Some(entry.password));
        }
    }
    Ok(None)
}

fn pgpass_path() -> Result<PathBuf, DriverError> {
    if let Ok(path) = std::env::var("PGPASSFILE") {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| DriverError::user("Password required", "no password and no HOME for .pgpass"))?;
    Ok(Path::new(&home).join(".pgpass"))
}

struct PgPassEntry {
    host: String,
    port: String,
    database: String,
    user: String,
    password: String,
}

impl PgPassEntry {
    fn matches(&self, host: &str, port: u16, database: &str, user: &str) -> bool {
        wildcard(&self.host, host)
            && (self.port == "*" || self.port == port.to_string())
            && wildcard(&self.database, database)
            && wildcard(&self.user, user)
    }
}

fn wildcard(pattern: &str, value: &str) -> bool {
    pattern == "*" || pattern == value
}

fn parse_line(line: &str) -> Option<PgPassEntry> {
    let mut parts = line.split(':');
    let host = parts.next()?.to_owned();
    let port = parts.next()?.to_owned();
    let database = parts.next()?.to_owned();
    let user = parts.next()?.to_owned();
    let password = parts.next()?.to_owned();
    Some(PgPassEntry {
        host,
        port,
        database,
        user,
        password,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_rules() {
        assert!(wildcard("*", "localhost"));
        assert!(wildcard("localhost", "localhost"));
        assert!(!wildcard("localhost", "127.0.0.1"));
    }
}
