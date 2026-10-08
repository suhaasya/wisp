//! Persisted main-window geometry (size, position, maximized).

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

const FILE_NAME: &str = "window.json";

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct WindowGeometry {
    pub width: f32,
    pub height: f32,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WindowState {
    #[serde(default)]
    pub maximized: bool,
    #[serde(default)]
    pub geometry: Option<WindowGeometry>,
}

impl WindowState {
    pub fn load() -> Self {
        let path = window_state_path();
        if !path.is_file() {
            return Self::default();
        }
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = window_state_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)
    }
}

pub fn window_state_path() -> PathBuf {
    crate::paths::WispPaths::resolve()
        .config_dir()
        .join(FILE_NAME)
}
