//! XDG JSON configuration repository.

use salah_domain::error::Error;
pub use salah_prayer::config::Config;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

pub fn path() -> PathBuf {
    path_from(
        env::var_os("SALAH_CONFIG"),
        env::var_os("XDG_CONFIG_HOME"),
        env::var_os("HOME"),
    )
}

fn path_from(
    explicit: Option<std::ffi::OsString>,
    xdg_config_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> PathBuf {
    if let Some(value) = explicit {
        return PathBuf::from(value);
    }
    let base = xdg_config_home
        .map(PathBuf::from)
        .or_else(|| home.map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("salah/config.json")
}

pub fn load_from(path: &Path) -> Result<Config, Error> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents = fs::read_to_string(path)
        .map_err(|e| Error::Config(format!("read {}: {e}", path.display())))?;
    let config: Config = serde_json::from_str(&contents)
        .map_err(|e| Error::Config(format!("parse {}: {e}", path.display())))?;
    config.validate()?;
    Ok(config)
}

pub fn load() -> Result<Config, Error> {
    load_from(&path())
}

pub fn save_to(config: &Config, path: &Path) -> Result<(), Error> {
    config.validate()?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|e| Error::Config(format!("create {}: {e}", parent.display())))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Config(format!("system clock: {e}")))?
        .as_nanos();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.json");
    let temp = parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));
    let mut bytes = serde_json::to_vec_pretty(config).map_err(|e| Error::Config(e.to_string()))?;
    bytes.push(b'\n');
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| Error::Config(format!("create {}: {e}", temp.display())))?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| Error::Config(format!("write {}: {e}", temp.display())))?;
        fs::rename(&temp, path)
            .map_err(|e| Error::Config(format!("replace {}: {e}", path.display())))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub fn save(config: &Config) -> Result<(), Error> {
    save_to(config, &path())
}

/// Records which settings apply to days that ended before this change.
pub fn record_change(
    tx: &crate::WriteTx<'_>,
    now: chrono::DateTime<chrono::Utc>,
    old: &Config,
    new: &Config,
) -> Result<(), Error> {
    let day = crate::day::current(old, now)?.max(crate::day::current(new, now)?);
    let body = serde_json::to_string(old).map_err(|e| Error::Internal(e.to_string()))?;
    tx.record_config_change(day, &body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn config_path_precedence_is_explicit_then_xdg_then_home() {
        assert_eq!(
            path_from(
                Some(OsString::from("/tmp/explicit.json")),
                Some(OsString::from("/tmp/xdg")),
                Some(OsString::from("/tmp/home")),
            ),
            PathBuf::from("/tmp/explicit.json")
        );
        assert_eq!(
            path_from(
                None,
                Some(OsString::from("/tmp/xdg")),
                Some(OsString::from("/tmp/home")),
            ),
            PathBuf::from("/tmp/xdg/salah/config.json")
        );
        assert_eq!(
            path_from(None, None, Some(OsString::from("/tmp/home"))),
            PathBuf::from("/tmp/home/.config/salah/config.json")
        );
    }
}
