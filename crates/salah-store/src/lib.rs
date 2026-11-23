//! SQLite persistence for Salah's v1 tracking model.

mod codec;
mod read;
mod write;

pub use write::WriteTx;

use chrono::{DateTime, Utc};
use codec::storage_error;
use rusqlite::Connection;
use salah_domain::error::Error;
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const SCHEMA: &str = include_str!("schema.sql");
const SCHEMA_VERSION: i64 = 1;
/// Attempts for a write that keeps hitting `SQLITE_BUSY`.
const WRITE_ATTEMPTS: u32 = 5;

pub fn schema_version() -> i64 {
    SCHEMA_VERSION
}

/// `$SALAH_DATA`, else `$XDG_DATA_HOME/salah/salah.db`, else `~/.local/share/salah/salah.db`.
pub fn default_path() -> PathBuf {
    if let Some(path) = std::env::var_os("SALAH_DATA").filter(|v| !v.is_empty()) {
        return path.into();
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| ".".into());
    base.join("salah/salah.db")
}

/// One row of the append-only change log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub seq: i64,
    pub revision: i64,
    pub entity: String,
    pub entity_id: String,
    pub op: String,
    pub version: i64,
    pub at: DateTime<Utc>,
    pub op_id: Option<String>,
}

/// The stored result of a client op id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedOp {
    pub method: String,
    pub revision: i64,
    pub result_json: String,
    pub at: DateTime<Utc>,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens (creating if needed) the database at `path`. A database from the
    /// pre-tracking schema is renamed to `<name>.old-<unix seconds>` and a
    /// fresh one is created in its place.
    pub fn open(path: &Path) -> Result<Self, Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Storage(format!("store: {e}")))?;
        }
        if path.exists() && is_old_schema(path)? {
            set_aside(path)?;
        }
        Self::init(Connection::open(path).map_err(storage_error)?)
    }

    pub fn open_in_memory() -> Result<Self, Error> {
        Self::init(Connection::open_in_memory().map_err(storage_error)?)
    }

    fn init(conn: Connection) -> Result<Self, Error> {
        conn.busy_timeout(Duration::from_secs(3))
            .map_err(storage_error)?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(storage_error)?;
        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(storage_error)?;
        let created = match user_version(&conn)? {
            0 => conn
                .execute_batch(SCHEMA)
                .and_then(|()| conn.pragma_update(None, "user_version", SCHEMA_VERSION))
                .map_err(storage_error),
            SCHEMA_VERSION => Ok(()),
            other => Err(Error::Storage(format!("store: unsupported schema {other}"))),
        };
        // These additive tables also install on existing tracking-v1 databases.
        let created = created.and_then(|()| {
            conn.execute_batch(include_str!("history.sql"))
                .map_err(storage_error)
        });
        match created {
            Ok(()) => conn.execute_batch("COMMIT").map_err(storage_error)?,
            Err(error) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(error);
            }
        }
        Ok(Self { conn })
    }

    pub fn version(&self) -> Result<i64, Error> {
        user_version(&self.conn)
    }

    /// The store-wide revision, bumped once per write transaction.
    pub fn revision(&self) -> Result<i64, Error> {
        self.conn
            .query_row("SELECT revision FROM meta", [], |row| row.get(0))
            .map_err(storage_error)
    }

    /// SQLite's `data_version`: changes when another connection commits.
    pub fn data_version(&self) -> Result<i64, Error> {
        self.conn
            .pragma_query_value(None, "data_version", |row| row.get(0))
            .map_err(storage_error)
    }

    pub fn foreign_keys_enabled(&self) -> Result<bool, Error> {
        self.conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .map_err(storage_error)
    }

    pub fn last_change_seq(&self) -> Result<i64, Error> {
        self.conn
            .query_row("SELECT COALESCE(MAX(seq), 0) FROM changes", [], |row| {
                row.get(0)
            })
            .map_err(storage_error)
    }

    /// Up to `limit` (at most 10 000) changes after `after_seq`, in order.
    pub fn changes_since(&self, after_seq: i64, limit: usize) -> Result<Vec<Change>, Error> {
        self.all(
            "SELECT * FROM changes WHERE seq > ?1 ORDER BY seq LIMIT ?2",
            rusqlite::params![after_seq, limit.min(10_000) as i64],
            codec::change,
        )
    }

    pub fn applied_op(&self, op_id: &str) -> Result<Option<AppliedOp>, Error> {
        self.one(
            "SELECT * FROM applied_ops WHERE op_id = ?1",
            [op_id],
            codec::applied_op,
        )
    }

    /// Runs `work` in one `BEGIN IMMEDIATE` transaction, committing on `Ok`.
    /// A `Busy` failure is retried with a short backoff.
    pub fn write<T>(
        &mut self,
        mut work: impl FnMut(&WriteTx<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut attempt = 0;
        loop {
            let transaction = rusqlite::Transaction::new_unchecked(
                &self.conn,
                rusqlite::TransactionBehavior::Immediate,
            )
            .map_err(storage_error)?;
            let tx = WriteTx {
                store: self,
                revision: Cell::new(None),
                op_id: RefCell::new(None),
            };
            match work(&tx) {
                Ok(value) => {
                    transaction.commit().map_err(storage_error)?;
                    return Ok(value);
                }
                Err(error) => {
                    transaction.rollback().map_err(storage_error)?;
                    attempt += 1;
                    if !matches!(error, Error::Busy(_)) || attempt == WRITE_ATTEMPTS {
                        return Err(error);
                    }
                    std::thread::sleep(Duration::from_millis(5 << attempt));
                }
            }
        }
    }
}

fn user_version(conn: &Connection) -> Result<i64, Error> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(storage_error)
}

/// A versioned database from before this schema: the planner schema, or an
/// unreleased tracking draft without the `days_written` table.
fn is_old_schema(path: &Path) -> Result<bool, Error> {
    let conn = Connection::open(path).map_err(storage_error)?;
    // Both facts must describe the same SQLite snapshot during first open.
    conn.query_row(
        "SELECT user_version > 0 AND NOT EXISTS (
            SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'days_written'
        ) FROM pragma_user_version",
        [],
        |row| row.get(0),
    )
    .map_err(storage_error)
}

fn set_aside(path: &Path) -> Result<(), Error> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("salah.db");
    std::fs::rename(path, path.with_file_name(format!("{name}.old-{seconds}")))
        .map_err(|e| Error::Storage(format!("store: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use chrono::TimeZone;
    use salah_domain::{
        day::{BlockKey, BoundaryId},
        item::Repeat,
        tracking::{Item, Project, Session, SessionValue},
    };
    fn t(h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2031, 5, 14, h, 0, 0).unwrap()
    }
    fn d() -> NaiveDate {
        NaiveDate::from_ymd_opt(2031, 5, 14).unwrap()
    }
    fn p() -> Project {
        Project {
            id: "p".into(),
            name: "P".into(),
            color: "#fff".into(),
            note: None,
            archived: false,
            tag_ids: vec![],
            created_at: t(1),
            updated_at: t(1),
            deleted_at: None,
            version: 1,
        }
    }
    fn i() -> Item {
        Item {
            id: "i".into(),
            title: "I".into(),
            project_id: Some("p".into()),
            block: BoundaryId::Fajr,
            repeat: Repeat::Daily,
            weekdays: None,
            day: d(),
            planned_minutes: Some(30),
            position: 0,
            created_at: t(1),
            updated_at: t(1),
            deleted_at: None,
            version: 1,
        }
    }
    #[test]
    fn schema_rows() {
        let mut s = Store::open_in_memory().unwrap();
        assert!(s.foreign_keys_enabled().unwrap());
        assert_eq!(s.metrics().unwrap().len(), 2);
        s.write(|x| {
            x.put_project(&p())?;
            x.put_item(&i())
        })
        .unwrap();
        assert_eq!(s.item("i").unwrap(), Some(i()));
        assert_eq!(s.revision().unwrap(), 1)
    }
    #[test]
    fn checks() {
        let mut s = Store::open_in_memory().unwrap();
        s.write(|x| x.put_project(&p())).unwrap();
        let mut x = i();
        x.planned_minutes = Some(-1);
        assert!(s.write(|q| q.put_item(&x)).is_err())
    }
    #[test]
    fn night_item_and_session_round_trip() {
        let mut s = Store::open_in_memory().unwrap();
        let mut item = i();
        item.block = BoundaryId::Night;
        let session = Session {
            id: "night-session".into(),
            item_id: Some(item.id.clone()),
            project_id: Some("p".into()),
            block: BlockKey {
                day: d(),
                start: BoundaryId::Night,
            },
            started_at: None,
            ended_at: None,
            minutes: Some(10),
            note: None,
            values: vec![],
            created_at: t(2),
            updated_at: t(2),
            deleted_at: None,
            version: 1,
        };
        s.write(|x| {
            x.put_project(&p())?;
            x.put_item(&item)?;
            x.put_session(&session)
        })
        .unwrap();
        assert_eq!(s.item(&item.id).unwrap(), Some(item));
        assert_eq!(s.session(&session.id).unwrap(), Some(session));
    }
    #[test]
    fn one_transaction_is_one_revision_with_a_change_per_entity() {
        let mut s = Store::open_in_memory().unwrap();
        let mut session = Session {
            id: "s".into(),
            item_id: None,
            project_id: Some("p".into()),
            block: BlockKey {
                day: d(),
                start: BoundaryId::Fajr,
            },
            started_at: None,
            ended_at: None,
            minutes: Some(25),
            note: Some("deep".into()),
            values: vec![SessionValue {
                metric_id: "focus".into(),
                value: 4.0,
            }],
            created_at: t(2),
            updated_at: t(2),
            deleted_at: None,
            version: 1,
        };
        s.write(|x| {
            x.set_op_id(Some("op-1"));
            x.put_project(&p())?;
            x.put_session(&session)?;
            x.record_applied_op("op-1", "item.add", "{}")
        })
        .unwrap();
        assert_eq!(s.revision().unwrap(), 1);
        assert_eq!(s.session("s").unwrap(), Some(session.clone()));
        assert_eq!(s.applied_op("op-1").unwrap().unwrap().revision, 1);
        session.deleted_at = Some(t(3));
        assert_eq!(s.write(|x| x.put_session(&session)).unwrap(), 2);
        let changes: Vec<_> = s
            .changes_since(0, 10)
            .unwrap()
            .into_iter()
            .map(|c| (c.revision, c.entity, c.op, c.op_id))
            .collect();
        assert_eq!(
            changes,
            [
                (1, "project".into(), "insert".into(), Some("op-1".into())),
                (1, "session".into(), "insert".into(), Some("op-1".into())),
                (2, "session".into(), "delete".into(), None),
            ]
        );
    }

    #[test]
    fn failed_commit_rolls_back_before_the_next_request() {
        let path =
            std::env::temp_dir().join(format!("salah-commit-busy-{}.db", std::process::id()));
        let mut store = Store::open(&path).unwrap();
        let reader = Connection::open(&path).unwrap();
        reader.execute_batch("BEGIN; SELECT * FROM meta").unwrap();
        store.conn.busy_timeout(Duration::ZERO).unwrap();
        let failed = store.write(|tx| tx.put_project(&p()));
        assert!(matches!(failed, Err(Error::Busy(_))));
        reader.execute_batch("ROLLBACK").unwrap();
        assert_eq!(store.revision().unwrap(), 0);
        assert!(store.project("p").unwrap().is_none());
        assert_eq!(store.write(|tx| tx.put_project(&p())).unwrap(), 1);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn tracking_v1_gets_history_tables_without_losing_applied_ops_or_snapshots() {
        let path = std::env::temp_dir().join(format!("salah-upgrade-{}.db", std::process::id()));
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch(
            "PRAGMA user_version=1;
            INSERT INTO days_written VALUES ('2031-05-14','2031-05-15T00:00:00Z');
            INSERT INTO applied_ops VALUES ('op','session.start',0,'{}','2031-05-14T00:00:00Z')",
        )
        .unwrap();
        drop(conn);
        let mut store = Store::open(&path).unwrap();
        assert_eq!(
            store.applied_op("op").unwrap().unwrap().method,
            "session.start"
        );
        assert!(store.day_written(d()).unwrap());
        assert!(store.day_clock(d()).unwrap().is_none());
        store
            .write(|tx| {
                tx.put_project(&p())?;
                tx.put_item(&i())?;
                tx.preserve_item(&i(), d())
            })
            .unwrap();
        assert_eq!(store.items_on(d()).unwrap(), [i()]);
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn old_database_is_renamed_aside() {
        let root = std::env::temp_dir().join(format!("salah-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("salah.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch("CREATE TABLE items(id TEXT); PRAGMA user_version=3;")
            .unwrap();
        drop(old);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.version().unwrap(), 1);
        assert!(std::fs::read_dir(&root).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("salah.db.old-")
        }));
        std::fs::remove_dir_all(root).unwrap();
    }
}
