//! Location sync policy: locking, throttling, and saving the config.

pub use salah_prayer::location::*;

use crate::config::{self, Config};
use salah_domain::error::Error;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::thread;
use std::time::Duration as StdDuration;

#[derive(Clone, Debug, Serialize)]
pub struct SyncOutcome {
    #[serde(flatten)]
    pub config: Config,
    pub updated: bool,
    pub reason: &'static str,
    pub provider: Option<String>,
}

struct SyncLock(PathBuf);

impl Drop for SyncLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn acquire_sync_lock() -> Result<SyncLock, Error> {
    let config_path = config::path();
    let parent = config_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    let lock_path = parent.join(".location-sync.lock");
    // Wait longer than curl's five-second ceiling so a second monitor does
    // not report a false failure while the first instance is still syncing.
    for _ in 0..140 {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let _ = writeln!(file, "{}", std::process::id());
                return Ok(SyncLock(lock_path));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let stale = fs::metadata(&lock_path)
                    .and_then(|metadata| metadata.modified())
                    .and_then(|modified| modified.elapsed().map_err(std::io::Error::other))
                    .is_ok_and(|age| age > StdDuration::from_secs(30));
                if stale {
                    let _ = fs::remove_file(&lock_path);
                } else {
                    thread::sleep(StdDuration::from_millis(50));
                }
            }
            Err(error) => return Err(format!("create {}: {error}", lock_path.display()).into()),
        }
    }
    Err(Error::Busy("another location sync is still running".into()))
}

pub fn sync_saved(
    if_needed: bool,
    now: chrono::DateTime<chrono::Utc>,
    local_timezone_offset: f64,
) -> Result<SyncOutcome, Error> {
    let _lock = acquire_sync_lock()?;
    // Reload only after taking the lock: another bar instance may have just
    // completed the same startup sync.
    let current = config::load()?;
    if if_needed && !current.auto_location {
        let provider = current.last_location_provider.clone();
        return Ok(SyncOutcome {
            config: current,
            updated: false,
            reason: "auto-location-disabled",
            provider,
        });
    }
    if if_needed && is_fresh(&current, now) {
        let provider = current.last_location_provider.clone();
        return Ok(SyncOutcome {
            config: current,
            updated: false,
            reason: "fresh",
            provider,
        });
    }
    let (updated, provider) = sync(&current, now, local_timezone_offset)?;
    config::save(&updated)?;
    Ok(SyncOutcome {
        config: updated,
        updated: true,
        reason: "updated",
        provider: Some(provider.to_string()),
    })
}
