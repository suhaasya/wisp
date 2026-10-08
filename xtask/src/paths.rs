use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{bail, Context, Result};

pub fn workspace_root() -> Result<PathBuf> {
    let output = Command::new("cargo")
        .args(["locate-project", "--workspace", "--message-format=plain"])
        .output()
        .context("failed to run cargo locate-project")?;
    if !output.status.success() {
        bail!("cargo locate-project failed");
    }
    let manifest = PathBuf::from(String::from_utf8(output.stdout)?.trim());
    manifest
        .parent()
        .map(Path::to_path_buf)
        .context("workspace manifest has no parent directory")
}

pub fn default_host_target() -> Option<String> {
    let output = Command::new("rustc")
        .args(["--version", "--verbose"])
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
}

pub fn release_binary_path(workspace_root: &Path, profile: &str, target: &str) -> Result<PathBuf> {
    let host = default_host_target().unwrap_or_default();
    let in_target_dir = target != host;
    let mut path = workspace_root.join("target");
    if in_target_dir {
        path.push(target);
    }
    path.push(profile);
    path.push(binary_file_name());
    Ok(path)
}

pub fn binary_file_name() -> &'static str {
    if cfg!(windows) {
        "wisp.exe"
    } else {
        "wisp"
    }
}

pub fn cargo_command(workspace_root: &Path) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(workspace_root);
    cmd
}
