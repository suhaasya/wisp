//! Minimal OpenSSH `~/.ssh/config` reader (Host, HostName, User, Port, IdentityFile).

use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Clone, Default)]
pub struct SshHostBlock {
    pub host_name: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct OpenSshConfig {
    blocks: HashMap<String, SshHostBlock>,
}

impl OpenSshConfig {
    pub fn load_default() -> std::io::Result<Self> {
        let path = home::home_dir()
            .map(|h| h.join(".ssh/config"))
            .ok_or_else(|| std::io::Error::other("no home directory"))?;
        Self::load_path(&path)
    }

    pub fn load_path(path: &std::path::Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        Ok(Self::parse(&text))
    }

    pub fn resolve(&self, host_token: &str) -> SshHostBlock {
        self.blocks
            .get(host_token)
            .cloned()
            .unwrap_or_default()
    }

    pub fn parse(text: &str) -> Self {
        let mut out = Self::default();
        let mut current_hosts: Vec<String> = Vec::new();
        let mut current = SshHostBlock::default();

        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            let Some(key) = parts.first() else {
                continue;
            };
            let value = parts[1..].join(" ");
            match key.to_ascii_lowercase().as_str() {
                "host" => {
                    if !current_hosts.is_empty() {
                        for h in current_hosts.drain(..) {
                            out.blocks.insert(h.to_ascii_lowercase(), current.clone());
                        }
                        current = SshHostBlock::default();
                    }
                    current_hosts.push(value);
                }
                "hostname" => current.host_name = Some(value),
                "user" => current.user = Some(value),
                "port" => {
                    if let Ok(port) = value.parse() {
                        current.port = Some(port);
                    }
                }
                "identityfile" => {
                    let expanded = expand_tilde(&value);
                    current.identity_file = Some(expanded);
                }
                _ => {}
            }
        }
        for h in current_hosts {
            out.blocks.insert(h.to_ascii_lowercase(), current.clone());
        }
        out
    }
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        home::home_dir()
            .map(|h| h.join(rest))
            .unwrap_or_else(|| PathBuf::from(path))
    } else {
        PathBuf::from(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_host_block() {
        let cfg = OpenSshConfig::parse(
            "Host bastion\n  HostName jump.example.com\n  User ubuntu\n  Port 2222\n  IdentityFile ~/.ssh/id_ed25519\n",
        );
        let block = cfg.resolve("bastion");
        assert_eq!(block.host_name.as_deref(), Some("jump.example.com"));
        assert_eq!(block.user.as_deref(), Some("ubuntu"));
        assert_eq!(block.port, Some(2222));
        assert!(block.identity_file.is_some());
    }
}
