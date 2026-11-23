//! Take a standalone SQLite snapshot, including committed WAL pages.

use crate::codec::storage_error;
use rusqlite::{Connection, MAIN_DB, OpenFlags};
use salah_domain::error::Error;
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const KEEP: usize = 3;

fn io_error(error: io::Error) -> Error {
    Error::Storage(format!("store: migration backup: {error}"))
}

fn directory(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

pub(super) fn lock_directory(path: &Path) -> Result<File, Error> {
    let dir = File::open(directory(path)).map_err(io_error)?;
    dir.lock().map_err(io_error)?;
    Ok(dir)
}

pub(super) struct Snapshot {
    path: PathBuf,
    obsolete: Vec<PathBuf>,
}

impl Snapshot {
    pub(super) fn finish(self, migration: Result<(), Error>) -> Result<(), Error> {
        if let Err(error) = migration {
            fs::remove_file(&self.path).map_err(|cleanup| {
                Error::Storage(format!(
                    "{error}. Could not discard backup {}: {cleanup}",
                    self.path.display()
                ))
            })?;
            return Err(error);
        }
        for path in self.obsolete {
            fs::remove_file(path).map_err(io_error)?;
        }
        Ok(())
    }
}

pub(super) fn save(path: &Path, version: i64) -> Result<Snapshot, Error> {
    let parent = directory(path);
    let name = path
        .file_name()
        .ok_or_else(|| Error::Storage("store: missing database filename".into()))?;
    let mut previous = Vec::new();
    for entry in fs::read_dir(parent).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let filename = entry.file_name();
        let Some(suffix) = filename
            .as_encoded_bytes()
            .strip_prefix(name.as_encoded_bytes())
        else {
            continue;
        };
        if let Some(base) = suffix.strip_suffix(b".partial") {
            if backup_time(base).is_some() && entry.file_type().map_err(io_error)?.is_file() {
                fs::remove_file(entry.path()).map_err(io_error)?;
            }
        } else if let Some(time) = backup_time(suffix)
            && entry.file_type().map_err(io_error)?.is_file()
        {
            previous.push((time, entry.path()));
        }
    }
    previous.sort_unstable();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let timestamp = previous
        .last()
        .map_or(now, |(last, _)| now.max(last.saturating_add(1)));
    let destination = path.with_added_extension(format!("bak-v{version}-{timestamp}"));
    if destination.try_exists().map_err(io_error)? {
        return Err(Error::Storage(
            "store: backup filename already exists".into(),
        ));
    }

    // A second, read-only connection sees the committed image while the
    // migration connection holds BEGIN IMMEDIATE against competing writers.
    let reader = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(storage_error)?;
    let partial = destination.with_added_extension("partial");
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&partial)
        .map_err(io_error)?;
    if let Err(error) = reader
        .backup(MAIN_DB, &partial, None)
        .map_err(storage_error)
        .and_then(|()| fs::rename(&partial, &destination).map_err(io_error))
    {
        let _ = fs::remove_file(partial);
        return Err(error);
    }
    // Make the completed snapshot's directory entry durable before migrating.
    let obsolete = previous.len().saturating_sub(KEEP - 1);
    let snapshot = Snapshot {
        path: destination,
        obsolete: previous
            .into_iter()
            .take(obsolete)
            .map(|(_, path)| path)
            .collect(),
    };
    if let Err(error) = File::open(parent).and_then(|dir| dir.sync_all()) {
        fs::remove_file(snapshot.path).map_err(io_error)?;
        return Err(io_error(error));
    }
    Ok(snapshot)
}

fn backup_time(suffix: &[u8]) -> Option<u128> {
    let (version, time) = std::str::from_utf8(suffix)
        .ok()?
        .strip_prefix(".bak-v")?
        .split_once('-')?;
    if version.is_empty()
        || !version.bytes().all(|b| b.is_ascii_digit())
        || !time.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    time.parse().ok()
}
