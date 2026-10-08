//! `known_hosts` verification with trust-on-first-use.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use russh::keys::PublicKey;
use russh::keys::ssh_key::{Fingerprint, HashAlg};

use super::error::SshError;

#[derive(Debug, Clone)]
pub struct HostKeyEntry {
    pub fingerprint_sha256: String,
    pub key_line: String,
}

pub fn fingerprint_sha256(key: &PublicKey) -> String {
    Fingerprint::new(HashAlg::Sha256, key.key_data()).to_string()
}

pub struct KnownHosts {
    path: PathBuf,
    entries: HashMap<String, HostKeyEntry>,
}

impl KnownHosts {
    pub fn wisp_default() -> std::io::Result<Self> {
        let path = wisp_store::WispPaths::resolve().ssh_known_hosts();
        Self::load(&path)
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        let mut entries = HashMap::new();
        if let Ok(text) = fs::read_to_string(path) {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let Some((host, _algo, _key)) = parse_known_hosts_line(line) else {
                    continue;
                };
                entries.insert(
                    host.to_string(),
                    HostKeyEntry {
                        fingerprint_sha256: String::new(),
                        key_line: line.to_string(),
                    },
                );
            }
        }
        Ok(Self {
            path: path.to_path_buf(),
            entries,
        })
    }

    pub fn verify_or_trust(
        &mut self,
        host: &str,
        port: u16,
        key: &PublicKey,
        allow_tofu: bool,
    ) -> Result<(), SshError> {
        let fp = fingerprint_sha256(key);
        let id = host_key_id(host, port);
        if let Some(entry) = self.entries.get(&id) {
            if verify_line_matches_key(&entry.key_line, key) {
                return Ok(());
            }
            return Err(SshError::HostKeyChanged {
                expected: entry.fingerprint_display(),
                actual: fp,
            });
        }
        if !allow_tofu {
            return Err(SshError::UnknownHostKey { fingerprint: fp });
        }
        let line = known_hosts_line(&id, key)?;
        self.entries.insert(
            id.clone(),
            HostKeyEntry {
                fingerprint_sha256: fp,
                key_line: line.clone(),
            },
        );
        self.persist()?;
        Ok(())
    }

    fn persist(&self) -> Result<(), SshError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body: String = self
            .entries
            .values()
            .map(|e| e.key_line.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&self.path, format!("{body}\n"))?;
        Ok(())
    }
}

impl HostKeyEntry {
    fn fingerprint_display(&self) -> String {
        if self.fingerprint_sha256.is_empty() {
            "stored key".into()
        } else {
            self.fingerprint_sha256.clone()
        }
    }
}

fn host_key_id(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    }
}

fn parse_known_hosts_line(line: &str) -> Option<(&str, &str, &str)> {
    let mut parts = line.split_whitespace();
    let host = parts.next()?;
    let algo = parts.next()?;
    let key = parts.next()?;
    Some((host, algo, key))
}

fn known_hosts_line(id: &str, key: &PublicKey) -> Result<String, SshError> {
    let openssh = key
        .to_openssh()
        .map_err(|e| SshError::Connect(e.to_string()))?;
    let mut parts = openssh.split_whitespace();
    let algo = parts
        .next()
        .ok_or_else(|| SshError::Connect("invalid server public key".into()))?;
    let b64 = parts
        .next()
        .ok_or_else(|| SshError::Connect("invalid server public key".into()))?;
    Ok(format!("{id} {algo} {b64}"))
}

fn verify_line_matches_key(line: &str, key: &PublicKey) -> bool {
    let Ok(expected) = known_hosts_line("host", key) else {
        return false;
    };
    let mut expected_parts = expected.split_whitespace();
    let _ = expected_parts.next();
    let expected_algo = expected_parts.next();
    let expected_b64 = expected_parts.next();
    line.split_whitespace().nth(1) == expected_algo
        && line.split_whitespace().nth(2) == expected_b64
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn rejects_mismatched_stored_key() {
        let key =
            PublicKey::from_openssh("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWiVhcvSv/9Fo")
                .expect("parse key");
        let other =
            PublicKey::from_openssh("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti")
                .expect("parse other");
        let line = known_hosts_line("bastion", &other).expect("line");
        let mut store = KnownHosts {
            path: std::env::temp_dir().join("wisp-kh-test"),
            entries: HashMap::from([(
                "bastion".into(),
                HostKeyEntry {
                    fingerprint_sha256: String::new(),
                    key_line: line,
                },
            )]),
        };
        let err = store
            .verify_or_trust("bastion", 22, &key, false)
            .expect_err("mismatch");
        assert!(matches!(err, SshError::HostKeyChanged { .. }));
    }

    #[test]
    fn fingerprint_uses_openssh_sha256_prefix() {
        let key =
            PublicKey::from_openssh("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWiVhcvSv/9Fo")
                .expect("parse key");
        let fp = fingerprint_sha256(&key);
        assert!(fp.starts_with("SHA256:"));
    }
}
