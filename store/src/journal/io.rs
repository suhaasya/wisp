//! Atomic journal read/write.

use std::{fs, io, path::Path};

use super::schema::{JOURNAL_VERSION, WindowJournal};

pub fn write_journal_atomic(path: &Path, doc: &WindowJournal) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec(doc).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn read_journal(path: &Path) -> io::Result<Option<WindowJournal>> {
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read(path)?;
    if raw.is_empty() {
        return Ok(None);
    }
    let doc: WindowJournal = match serde_json::from_slice(&raw) {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };
    if doc.version != JOURNAL_VERSION {
        return Ok(None);
    }
    Ok(Some(doc))
}

pub fn remove_journal(path: &Path) -> io::Result<()> {
    if path.is_file() {
        fs::remove_file(path)?;
    }
    Ok(())
}
