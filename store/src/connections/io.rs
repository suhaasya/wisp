//! Atomic read/write for `connections.toml`.

use std::{fs, io, path::Path};

use super::schema::ConnectionsFile;

pub fn write_connections_atomic(path: &Path, file: &ConnectionsFile) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(file).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, e.to_string())
    })?;
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, text)?;
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn backup_corrupt_file(path: &Path) -> io::Result<std::path::PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup = path.with_extension(format!("toml.corrupt-{stamp}"));
    fs::copy(path, &backup)?;
    Ok(backup)
}
