use crate::error::Error;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Config {
    pub location: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone_offset: f64,
    pub method: String,
    pub asr: String,
    #[serde(default = "default_sleep_time")]
    pub sleep_time: String,
    #[serde(default = "default_auto_location")]
    pub auto_location: bool,
    #[serde(default)]
    pub last_location_sync: Option<String>,
    #[serde(default)]
    pub last_location_provider: Option<String>,
}

const fn default_auto_location() -> bool {
    true
}

fn default_sleep_time() -> String {
    "23:00".into()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            location: "Tashkent".into(),
            latitude: 41.2995,
            longitude: 69.2401,
            timezone_offset: 5.0,
            method: "uzbekistan".into(),
            asr: "hanafi".into(),
            sleep_time: default_sleep_time(),
            auto_location: true,
            last_location_sync: None,
            last_location_provider: None,
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), Error> {
        if self.location.trim().is_empty() {
            return Err("location cannot be empty".into());
        }
        if !(-90.0..=90.0).contains(&self.latitude) {
            return Err("latitude must be between -90 and 90".into());
        }
        if !(-180.0..=180.0).contains(&self.longitude) {
            return Err("longitude must be between -180 and 180".into());
        }
        if !(-14.0..=14.0).contains(&self.timezone_offset) {
            return Err("timezone offset must be between -14 and 14".into());
        }
        if crate::calculation::method(&self.method).is_none() {
            return Err(format!("unknown method: {}", self.method).into());
        }
        if self.asr != "standard" && self.asr != "hanafi" {
            return Err("asr must be standard or hanafi".into());
        }
        crate::day::parse_clock_minutes(&self.sleep_time)
            .map_err(|_| "sleep time must use 24-hour HH:MM format".to_string())?;
        if let Some(timestamp) = &self.last_location_sync
            && (timestamp.len() > 64
                || timestamp.chars().any(char::is_control)
                || chrono::DateTime::parse_from_rfc3339(timestamp).is_err())
        {
            return Err("invalid last location sync timestamp".into());
        }
        if let Some(provider) = &self.last_location_provider
            && provider != "geo.omarchy.org"
            && provider != "ipapi.co"
            && provider != "omarchy-weather"
        {
            return Err("invalid location provider".into());
        }
        Ok(())
    }
}

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

    #[test]
    fn non_finite_numbers_are_rejected() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let config = Config {
                latitude: value,
                ..Config::default()
            };
            assert!(config.validate().is_err());
            let config = Config {
                longitude: value,
                ..Config::default()
            };
            assert!(config.validate().is_err());
            let config = Config {
                timezone_offset: value,
                ..Config::default()
            };
            assert!(config.validate().is_err());
        }
    }
}
