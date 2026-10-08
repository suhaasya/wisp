//! Per-OS config, data, and cache directories (`directories` crate).

use std::path::{Path, PathBuf};

const QUALIFIER: &str = "";
const ORGANIZATION: &str = "";
const APPLICATION: &str = "wisp";

/// Resolved Wisp directories for the current OS user.
#[derive(Debug, Clone)]
pub struct WispPaths {
    config_dir: PathBuf,
    data_dir: PathBuf,
    cache_dir: PathBuf,
}

impl WispPaths {
    pub fn resolve() -> Self {
        if let Some(dirs) = directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION) {
            Self {
                config_dir: dirs.config_dir().to_path_buf(),
                data_dir: dirs.data_dir().to_path_buf(),
                cache_dir: dirs.cache_dir().to_path_buf(),
            }
        } else {
            let base = PathBuf::from(".wisp");
            Self {
                config_dir: base.join("config"),
                data_dir: base.join("data"),
                cache_dir: base.join("cache"),
            }
        }
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn settings_toml(&self) -> PathBuf {
        self.config_dir.join("settings.toml")
    }

    pub fn connections_toml(&self) -> PathBuf {
        self.config_dir.join("connections.toml")
    }
}

impl Default for WispPaths {
    fn default() -> Self {
        Self::resolve()
    }
}

impl WispPaths {
    /// Override directories (tests and isolated runs).
    pub fn from_dirs(config_dir: PathBuf, data_dir: PathBuf, cache_dir: PathBuf) -> Self {
        Self {
            config_dir,
            data_dir,
            cache_dir,
        }
    }
}
