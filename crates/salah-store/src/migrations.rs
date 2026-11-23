//! Each migration commits its SQL and version bump in one transaction.

mod backup;
#[cfg(test)]
mod tests;

use crate::{codec::storage_error, user_version};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};
use salah_domain::error::Error;
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) struct Step {
    sql: &'static str,
    foreign_keys_off: bool,
}

pub(super) const STEPS: &[Step] = &[
    Step {
        sql: include_str!("migrations/001_schema.sql"),
        foreign_keys_off: false,
    },
    Step {
        sql: include_str!("migrations/002_history.sql"),
        foreign_keys_off: false,
    },
    Step {
        sql: include_str!("migrations/003_tracking_ui.sql"),
        foreign_keys_off: false,
    },
    Step {
        sql: include_str!("migrations/004_epics_backlog_resources.sql"),
        foreign_keys_off: true,
    },
];

pub(super) fn apply(conn: &mut Connection, path: Option<&Path>) -> Result<(), Error> {
    run(conn, path, STEPS)
}

fn run(conn: &mut Connection, path: Option<&Path>, steps: &[Step]) -> Result<(), Error> {
    if recognized_version(conn, steps.len())? == steps.len() {
        return Ok(());
    }
    // SQLite releases its writer lock at commit/rollback. Serialize backup
    // publication and its subsequent prune/discard across that boundary too.
    let _backup_lock = path.map(backup::lock_directory).transpose()?;
    loop {
        let version = recognized_version(conn, steps.len())?;
        let Some(step) = steps.get(version) else {
            return Ok(());
        };
        if step.foreign_keys_off {
            conn.pragma_update(None, "foreign_keys", false)
                .map_err(storage_error)?;
        }
        let result = step.apply(conn, path, version as i64);
        if step.foreign_keys_off {
            conn.pragma_update(None, "foreign_keys", true)
                .map_err(storage_error)?;
        }
        result?;
    }
}

fn recognized_version(conn: &Connection, latest: usize) -> Result<usize, Error> {
    // Read both facts in one snapshot, including during concurrent first opens.
    let (version, tracking): (i64, bool) = conn
        .query_row(
            "SELECT user_version, EXISTS(SELECT 1 FROM sqlite_schema
         WHERE type='table' AND name='days_written') FROM pragma_user_version",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(storage_error)?;
    if !(0..=latest as i64).contains(&version) {
        return Err(Error::Storage(format!(
            "store: found unsupported schema version {version} (latest supported is {latest}). The file was left untouched."
        )));
    }
    if version > 0 && !tracking {
        return Err(Error::Storage(format!(
            "store: found a pre-tracking or unrecognized schema at version {version} (missing days_written table). The file was left untouched."
        )));
    }
    Ok(version as usize)
}

impl Step {
    fn apply(&self, conn: &mut Connection, path: Option<&Path>, version: i64) -> Result<(), Error> {
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage_error)?;
        if user_version(&tx)? != version {
            // Another migrator advanced the database before BEGIN.
            return Ok(());
        }
        let snapshot = match path {
            Some(path) if version > 0 => Some(backup::save(path, version)?),
            _ => None,
        };
        let result = self.commit(tx, version + 1);
        match snapshot {
            Some(snapshot) => snapshot.finish(result),
            None => result,
        }
    }

    fn commit(&self, tx: Transaction<'_>, next: i64) -> Result<(), Error> {
        tx.execute_batch(self.sql).map_err(storage_error)?;
        if self.foreign_keys_off
            && let Some(table) = tx
                .query_row("PRAGMA foreign_key_check", [], |r| r.get::<_, String>(0))
                .optional()
                .map_err(storage_error)?
        {
            return Err(Error::Storage(format!(
                "store: migration {next} violates a foreign key in {table}"
            )));
        }
        tx.pragma_update(None, "user_version", next)
            .map_err(storage_error)?;
        tx.commit().map_err(storage_error)
    }
}
