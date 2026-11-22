//! Local SQLite persistence for items and entries.
//!
//! The store only saves and loads rows; planning rules live in `item` and
//! `planner`. Rows are never physically deleted: `deleted_at` tombstones let
//! deletions travel through export/import and, later, sync.

use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use salah_domain::day::BlockKey;
use salah_domain::error::Error;
use salah_domain::item::{Entry, EntryKind, Item, Measure, PrayerMark, Repeat, Weekdays};
use serde::{Serialize, de::DeserializeOwned};
use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How many times a busy write transaction is retried before `Error::Busy`
/// reaches the caller. SQLite's own busy timeout already waits 3 s per
/// attempt; these short retries cover lock hand-offs between processes.
const WRITE_ATTEMPTS: u32 = 5;

/// Each entry upgrades the schema by one version. Never edit a released
/// migration; append a new one.
const MIGRATIONS: &[&str] = &[
    "
    CREATE TABLE items (
        id          TEXT PRIMARY KEY,
        title       TEXT NOT NULL,
        block       TEXT NOT NULL,
        repeat      TEXT NOT NULL,
        measure     TEXT NOT NULL,
        target      INTEGER,
        day         TEXT NOT NULL,
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL,
        deleted_at  TEXT
    );
    CREATE TABLE entries (
        id          TEXT PRIMARY KEY,
        item_id     TEXT NOT NULL,
        day         TEXT NOT NULL,
        block       TEXT NOT NULL,
        kind        TEXT NOT NULL,
        value       INTEGER,
        started_at  TEXT,
        ended_at    TEXT,
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL,
        deleted_at  TEXT
    );
    CREATE INDEX entries_by_day ON entries(day);
    CREATE INDEX entries_by_item ON entries(item_id);
",
    "
    ALTER TABLE items ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE items ADD COLUMN weekdays INTEGER;
    CREATE TABLE prayer_marks (
        id          TEXT PRIMARY KEY,
        day         TEXT NOT NULL,
        prayer      TEXT NOT NULL,
        marked_at   TEXT NOT NULL,
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL,
        deleted_at  TEXT
    );
    CREATE INDEX prayer_marks_by_day ON prayer_marks(day);
",
    "
    ALTER TABLE items ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE entries ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE prayer_marks ADD COLUMN version INTEGER NOT NULL DEFAULT 1;

    CREATE TABLE meta (
        id          INTEGER PRIMARY KEY CHECK (id = 1),
        revision    INTEGER NOT NULL DEFAULT 0
    );
    INSERT INTO meta (id, revision) VALUES (1, 0);

    CREATE TABLE changes (
        seq         INTEGER PRIMARY KEY AUTOINCREMENT,
        revision    INTEGER NOT NULL,
        entity      TEXT NOT NULL,
        entity_id   TEXT NOT NULL,
        op          TEXT NOT NULL CHECK (op IN ('insert', 'update', 'delete')),
        version     INTEGER NOT NULL,
        at          TEXT NOT NULL,
        op_id       TEXT
    );
    CREATE INDEX changes_by_revision ON changes(revision);
    CREATE INDEX changes_by_entity ON changes(entity, entity_id);

    CREATE TABLE applied_ops (
        op_id       TEXT PRIMARY KEY,
        revision    INTEGER NOT NULL,
        result_json TEXT NOT NULL,
        at          TEXT NOT NULL
    );
",
];

pub fn schema_version() -> i64 {
    MIGRATIONS.len() as i64
}

/// `$SALAH_DATA`, else `${XDG_DATA_HOME:-~/.local/share}/salah/salah.db`.
pub fn default_path() -> PathBuf {
    if let Some(path) = std::env::var_os("SALAH_DATA").filter(|value| !value.is_empty()) {
        return PathBuf::from(path);
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("salah").join("salah.db")
}

pub struct Store {
    conn: Connection,
}

fn sqlite_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(details, _)
            if matches!(
                details.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
    )
}

fn sqlite_error(error: rusqlite::Error) -> Error {
    let message = format!("store: {error}");
    if sqlite_busy(&error) {
        Error::Busy(message)
    } else {
        Error::Storage(message)
    }
}

fn retry_delay(attempt: u32) -> Duration {
    Duration::from_millis(5u64 << attempt)
}

/// Switching a fresh database to WAL can return `SQLITE_BUSY` immediately,
/// before SQLite's busy handler runs, so retry this step explicitly too.
fn set_journal_mode(conn: &Connection) -> Result<(), Error> {
    for attempt in 0..WRITE_ATTEMPTS {
        match conn.pragma_update(None, "journal_mode", "WAL") {
            Ok(()) => return Ok(()),
            Err(error) if sqlite_busy(&error) && attempt + 1 < WRITE_ATTEMPTS => {
                std::thread::sleep(retry_delay(attempt));
            }
            Err(error) => return Err(sqlite_error(error)),
        }
    }
    unreachable!("the loop returns on its last attempt")
}

fn time_text(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn parse_time(text: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
        })
}

fn parse_opt_time(text: Option<String>) -> rusqlite::Result<Option<DateTime<Utc>>> {
    text.as_deref().map(parse_time).transpose()
}

fn parse_day(text: &str) -> rusqlite::Result<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
    })
}

/// Enums are stored as their serde names so the database, exports, and JSON
/// output use one vocabulary.
fn enum_text<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(text)) => text,
        _ => unreachable!("stored enums serialize as strings"),
    }
}

fn parse_enum<T: DeserializeOwned>(text: &str) -> rusqlite::Result<T> {
    serde_json::from_value(serde_json::Value::String(text.to_string())).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
    })
}

fn item_from(row: &Row) -> rusqlite::Result<Item> {
    Ok(Item {
        id: row.get("id")?,
        title: row.get("title")?,
        block: parse_enum(&row.get::<_, String>("block")?)?,
        repeat: parse_enum::<Repeat>(&row.get::<_, String>("repeat")?)?,
        weekdays: row.get::<_, Option<u8>>("weekdays")?.map(Weekdays),
        measure: parse_enum::<Measure>(&row.get::<_, String>("measure")?)?,
        target: row.get("target")?,
        position: row.get("position")?,
        day: parse_day(&row.get::<_, String>("day")?)?,
        created_at: parse_time(&row.get::<_, String>("created_at")?)?,
        updated_at: parse_time(&row.get::<_, String>("updated_at")?)?,
        deleted_at: parse_opt_time(row.get("deleted_at")?)?,
        version: row.get("version")?,
    })
}

fn mark_from(row: &Row) -> rusqlite::Result<PrayerMark> {
    Ok(PrayerMark {
        id: row.get("id")?,
        day: parse_day(&row.get::<_, String>("day")?)?,
        prayer: parse_enum(&row.get::<_, String>("prayer")?)?,
        marked_at: parse_time(&row.get::<_, String>("marked_at")?)?,
        created_at: parse_time(&row.get::<_, String>("created_at")?)?,
        updated_at: parse_time(&row.get::<_, String>("updated_at")?)?,
        deleted_at: parse_opt_time(row.get("deleted_at")?)?,
        version: row.get("version")?,
    })
}

fn entry_from(row: &Row) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id: row.get("id")?,
        item_id: row.get("item_id")?,
        block: BlockKey {
            day: parse_day(&row.get::<_, String>("day")?)?,
            start: parse_enum(&row.get::<_, String>("block")?)?,
        },
        kind: parse_enum::<EntryKind>(&row.get::<_, String>("kind")?)?,
        value: row.get("value")?,
        started_at: parse_opt_time(row.get("started_at")?)?,
        ended_at: parse_opt_time(row.get("ended_at")?)?,
        created_at: parse_time(&row.get::<_, String>("created_at")?)?,
        updated_at: parse_time(&row.get::<_, String>("updated_at")?)?,
        deleted_at: parse_opt_time(row.get("deleted_at")?)?,
        version: row.get("version")?,
    })
}

/// One row of the append-only change log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub seq: i64,
    pub revision: i64,
    pub entity: String,
    pub entity_id: String,
    pub op: String,
    /// Version of the entity after the change.
    pub version: i64,
    pub at: DateTime<Utc>,
    pub op_id: Option<String>,
}

/// A stored idempotent mutation result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedOp {
    pub revision: i64,
    pub result_json: String,
    pub at: DateTime<Utc>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store, Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| Error::Storage(format!("store: {error}")))?;
        }
        let conn = Connection::open(path).map_err(sqlite_error)?;
        // Set the timeout before touching journal mode so two processes
        // opening a fresh database wait for each other instead of failing.
        conn.busy_timeout(Duration::from_secs(3))
            .map_err(sqlite_error)?;
        // WAL lets the widget read while the app or CLI writes.
        set_journal_mode(&conn)?;
        Store::init(conn)
    }

    pub fn open_in_memory() -> Result<Store, Error> {
        Store::init(Connection::open_in_memory().map_err(sqlite_error)?)
    }

    fn init(conn: Connection) -> Result<Store, Error> {
        conn.busy_timeout(Duration::from_secs(3))
            .map_err(sqlite_error)?;
        let mut store = Store { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn version(&self) -> Result<i64, Error> {
        self.conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(sqlite_error)
    }

    /// The store-wide revision. It advances by exactly one for each committed
    /// transaction that appends a change.
    pub fn revision(&self) -> Result<i64, Error> {
        self.conn
            .query_row("SELECT revision FROM meta WHERE id = 1", [], |row| {
                row.get(0)
            })
            .map_err(sqlite_error)
    }

    /// Changes with `seq` greater than `after_seq`, oldest first.
    pub fn changes_since(&self, after_seq: i64, limit: usize) -> Result<Vec<Change>, Error> {
        let limit = limit.min(10_000) as i64;
        let mut statement = self
            .conn
            .prepare(
                "SELECT seq, revision, entity, entity_id, op, version, at, op_id
                 FROM changes WHERE seq > ?1 ORDER BY seq LIMIT ?2",
            )
            .map_err(sqlite_error)?;
        statement
            .query_map(params![after_seq, limit], |row| {
                Ok(Change {
                    seq: row.get("seq")?,
                    revision: row.get("revision")?,
                    entity: row.get("entity")?,
                    entity_id: row.get("entity_id")?,
                    op: row.get("op")?,
                    version: row.get("version")?,
                    at: parse_time(&row.get::<_, String>("at")?)?,
                    op_id: row.get("op_id")?,
                })
            })
            .map_err(sqlite_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_error)
    }

    /// The stored result of `op_id`, if that mutation was already applied.
    pub fn applied_op(&self, op_id: &str) -> Result<Option<AppliedOp>, Error> {
        self.conn
            .query_row(
                "SELECT revision, result_json, at FROM applied_ops WHERE op_id = ?1",
                [op_id],
                |row| {
                    Ok(AppliedOp {
                        revision: row.get("revision")?,
                        result_json: row.get("result_json")?,
                        at: parse_time(&row.get::<_, String>("at")?)?,
                    })
                },
            )
            .optional()
            .map_err(sqlite_error)
    }

    fn migrate(&mut self) -> Result<(), Error> {
        let current = self.version()?;
        if current > schema_version() {
            return Err(format!(
                "store: database schema {current} is newer than this salah ({}); update salah",
                schema_version()
            )
            .into());
        }
        // Re-read the version under the write lock: another process may have
        // migrated the database while this one was waiting for the lock.
        while self.version()? < schema_version() {
            self.write(|tx| {
                let current = tx.version()?;
                if current > schema_version() {
                    return Err(format!(
                        "store: database schema {current} is newer than this salah ({}); update salah",
                        schema_version()
                    )
                    .into());
                }
                if let Some(sql) = MIGRATIONS.get(current as usize) {
                    tx.store.conn.execute_batch(sql).map_err(sqlite_error)?;
                    tx.store
                        .conn
                        .pragma_update(None, "user_version", current + 1)
                        .map_err(sqlite_error)?;
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Runs `work` in one `BEGIN IMMEDIATE` transaction. Every read the use
    /// case needs must happen through the handle, so no read-then-write can
    /// straddle transactions. Nothing is saved if `work` fails.
    ///
    /// A busy database retries the whole transaction a bounded number of times
    /// with short backoff; after that `Error::Busy` is returned.
    pub fn write<T>(
        &mut self,
        mut work: impl FnMut(&WriteTx<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        for attempt in 0..WRITE_ATTEMPTS {
            match self.write_once(&mut work) {
                Ok(value) => return Ok(value),
                Err(Error::Busy(_)) if attempt + 1 < WRITE_ATTEMPTS => {
                    std::thread::sleep(retry_delay(attempt));
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!("the loop returns on its last attempt")
    }

    fn write_once<T>(
        &self,
        work: &mut impl FnMut(&WriteTx<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.conn
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(sqlite_error)?;
        let tx = WriteTx {
            store: self,
            revision: Cell::new(None),
            op_id: RefCell::new(None),
        };
        let value = match work(&tx) {
            Ok(value) => value,
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                return Err(error);
            }
        };
        match self.conn.execute_batch("COMMIT") {
            Ok(()) => Ok(value),
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(sqlite_error(error))
            }
        }
    }

    pub fn mark(&self, id: &str) -> Result<Option<PrayerMark>, Error> {
        self.conn
            .query_row("SELECT * FROM prayer_marks WHERE id = ?1", [id], mark_from)
            .optional()
            .map_err(sqlite_error)
    }

    /// Every prayer mark, deleted ones included, oldest first.
    pub fn marks(&self) -> Result<Vec<PrayerMark>, Error> {
        self.query(
            "SELECT * FROM prayer_marks ORDER BY created_at, id",
            [],
            mark_from,
        )
    }

    /// Prayer marks for days in the inclusive range.
    pub fn marks_between(
        &self,
        first: NaiveDate,
        last: NaiveDate,
    ) -> Result<Vec<PrayerMark>, Error> {
        self.query(
            "SELECT * FROM prayer_marks WHERE day BETWEEN ?1 AND ?2 ORDER BY day, id",
            [
                first.format("%Y-%m-%d").to_string(),
                last.format("%Y-%m-%d").to_string(),
            ],
            mark_from,
        )
    }

    pub fn item(&self, id: &str) -> Result<Option<Item>, Error> {
        self.conn
            .query_row("SELECT * FROM items WHERE id = ?1", [id], item_from)
            .optional()
            .map_err(sqlite_error)
    }

    pub fn entry(&self, id: &str) -> Result<Option<Entry>, Error> {
        self.conn
            .query_row("SELECT * FROM entries WHERE id = ?1", [id], entry_from)
            .optional()
            .map_err(sqlite_error)
    }

    /// Every item, deleted ones included, oldest first.
    pub fn items(&self) -> Result<Vec<Item>, Error> {
        self.query("SELECT * FROM items ORDER BY created_at, id", [], item_from)
    }

    /// Every entry, deleted ones included, oldest first.
    pub fn entries(&self) -> Result<Vec<Entry>, Error> {
        self.query(
            "SELECT * FROM entries ORDER BY created_at, id",
            [],
            entry_from,
        )
    }

    /// Entries whose block lies in the inclusive day range.
    pub fn entries_between(&self, first: NaiveDate, last: NaiveDate) -> Result<Vec<Entry>, Error> {
        self.query(
            "SELECT * FROM entries WHERE day BETWEEN ?1 AND ?2 ORDER BY created_at, id",
            [
                first.format("%Y-%m-%d").to_string(),
                last.format("%Y-%m-%d").to_string(),
            ],
            entry_from,
        )
    }

    pub fn running_timers(&self) -> Result<Vec<Entry>, Error> {
        self.query(
            "SELECT * FROM entries
             WHERE kind = 'timer' AND ended_at IS NULL AND deleted_at IS NULL",
            [],
            entry_from,
        )
    }

    fn query<T, P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
        map: fn(&Row) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>, Error> {
        let mut statement = self.conn.prepare(sql).map_err(sqlite_error)?;
        statement
            .query_map(params, map)
            .map_err(sqlite_error)?
            .collect::<rusqlite::Result<Vec<T>>>()
            .map_err(sqlite_error)
    }
}

/// A transaction handle. It exposes the store's reads (through `Deref`) and
/// the only write helpers, so a mutation cannot escape its transaction.
///
/// Every successful entity write appends a `changes` row and, on the first
/// change of the transaction, advances the global revision exactly once.
pub struct WriteTx<'a> {
    store: &'a Store,
    revision: Cell<Option<i64>>,
    op_id: RefCell<Option<String>>,
}

impl Deref for WriteTx<'_> {
    type Target = Store;

    fn deref(&self) -> &Store {
        self.store
    }
}

impl WriteTx<'_> {
    /// The idempotency key that all changes in this transaction belong to.
    pub fn set_op_id(&self, op_id: Option<&str>) {
        *self.op_id.borrow_mut() = op_id.map(str::to_owned);
    }

    fn current_revision(&self) -> Result<i64, Error> {
        match self.revision.get() {
            Some(revision) => Ok(revision),
            None => self.store.revision(),
        }
    }

    fn next_revision(&self) -> Result<i64, Error> {
        if let Some(revision) = self.revision.get() {
            return Ok(revision);
        }
        let revision = self
            .store
            .conn
            .query_row(
                "UPDATE meta SET revision = revision + 1 WHERE id = 1 RETURNING revision",
                [],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        self.revision.set(Some(revision));
        Ok(revision)
    }

    /// Stores an idempotent mutation result. This is metadata, so it does not
    /// advance the revision on its own.
    pub fn record_applied_op(&self, op_id: &str, result_json: &str) -> Result<(), Error> {
        let revision = self.current_revision()?;
        self.store
            .conn
            .execute(
                "INSERT INTO applied_ops (op_id, revision, result_json, at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(op_id) DO NOTHING",
                params![op_id, revision, result_json, time_text(Utc::now())],
            )
            .map(|_| ())
            .map_err(sqlite_error)
    }

    fn append_change(
        &self,
        entity: &str,
        entity_id: &str,
        op: &str,
        version: i64,
    ) -> Result<(), Error> {
        let revision = self.next_revision()?;
        let at = time_text(Utc::now());
        let op_id = self.op_id.borrow();
        self.store
            .conn
            .execute(
                "INSERT INTO changes (revision, entity, entity_id, op, version, at, op_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    revision,
                    entity,
                    entity_id,
                    op,
                    version,
                    at,
                    op_id.as_deref()
                ],
            )
            .map(|_| ())
            .map_err(sqlite_error)
    }

    fn existing_version(
        &self,
        table: &str,
        id: &str,
    ) -> Result<Option<(i64, Option<String>)>, Error> {
        // `table` is one of the three literals below, never user input.
        self.store
            .conn
            .query_row(
                &format!("SELECT version, deleted_at FROM {table} WHERE id = ?1"),
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(sqlite_error)
    }

    /// Chooses the next version and the change operation. A tombstone
    /// transition is a `delete`; every other existing-row write is an
    /// `update`.
    fn version_for(
        &self,
        table: &str,
        id: &str,
        deleted_at: Option<DateTime<Utc>>,
        explicit: Option<i64>,
    ) -> Result<(i64, &'static str), Error> {
        match self.existing_version(table, id)? {
            None => Ok((explicit.unwrap_or(1).max(1), "insert")),
            Some((current, was_deleted)) => {
                let op = if deleted_at.is_some() && was_deleted.is_none() {
                    "delete"
                } else {
                    "update"
                };
                let version = match explicit {
                    Some(version) => version.max(current + 1),
                    None => current + 1,
                };
                Ok((version, op))
            }
        }
    }

    fn write_item(&self, item: &Item, explicit: Option<i64>) -> Result<i64, Error> {
        let (version, op) = self.version_for("items", &item.id, item.deleted_at, explicit)?;
        self.store
            .conn
            .execute(
                "INSERT INTO items (id, title, block, repeat, measure, target, day,
                                    created_at, updated_at, deleted_at, position, weekdays,
                                    version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title, block = excluded.block,
                    repeat = excluded.repeat, measure = excluded.measure,
                    target = excluded.target, day = excluded.day,
                    position = excluded.position, weekdays = excluded.weekdays,
                    created_at = excluded.created_at, updated_at = excluded.updated_at,
                    deleted_at = excluded.deleted_at, version = excluded.version",
                params![
                    item.id,
                    item.title,
                    enum_text(&item.block),
                    enum_text(&item.repeat),
                    enum_text(&item.measure),
                    item.target,
                    item.day.format("%Y-%m-%d").to_string(),
                    time_text(item.created_at),
                    time_text(item.updated_at),
                    item.deleted_at.map(time_text),
                    item.position,
                    item.weekdays.map(|w| w.0),
                    version,
                ],
            )
            .map_err(sqlite_error)?;
        self.append_change("item", &item.id, op, version)?;
        Ok(version)
    }

    pub fn put_item(&self, item: &Item) -> Result<i64, Error> {
        self.write_item(item, None)
    }

    /// Stores a row with an imported version instead of bumping the local one.
    /// The version still never moves backwards for an existing row.
    pub fn put_item_versioned(&self, item: &Item, version: i64) -> Result<i64, Error> {
        self.write_item(item, Some(version))
    }

    fn write_entry(&self, entry: &Entry, explicit: Option<i64>) -> Result<i64, Error> {
        let (version, op) = self.version_for("entries", &entry.id, entry.deleted_at, explicit)?;
        self.store
            .conn
            .execute(
                "INSERT INTO entries (id, item_id, day, block, kind, value, started_at,
                                      ended_at, created_at, updated_at, deleted_at, version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(id) DO UPDATE SET
                    item_id = excluded.item_id, day = excluded.day,
                    block = excluded.block, kind = excluded.kind,
                    value = excluded.value, started_at = excluded.started_at,
                    ended_at = excluded.ended_at, created_at = excluded.created_at,
                    updated_at = excluded.updated_at, deleted_at = excluded.deleted_at,
                    version = excluded.version",
                params![
                    entry.id,
                    entry.item_id,
                    entry.block.day.format("%Y-%m-%d").to_string(),
                    enum_text(&entry.block.start),
                    enum_text(&entry.kind),
                    entry.value,
                    entry.started_at.map(time_text),
                    entry.ended_at.map(time_text),
                    time_text(entry.created_at),
                    time_text(entry.updated_at),
                    entry.deleted_at.map(time_text),
                    version,
                ],
            )
            .map_err(sqlite_error)?;
        self.append_change("entry", &entry.id, op, version)?;
        Ok(version)
    }

    pub fn put_entry(&self, entry: &Entry) -> Result<i64, Error> {
        self.write_entry(entry, None)
    }

    pub fn put_entry_versioned(&self, entry: &Entry, version: i64) -> Result<i64, Error> {
        self.write_entry(entry, Some(version))
    }

    fn write_mark(&self, mark: &PrayerMark, explicit: Option<i64>) -> Result<i64, Error> {
        let (version, op) =
            self.version_for("prayer_marks", &mark.id, mark.deleted_at, explicit)?;
        self.store
            .conn
            .execute(
                "INSERT INTO prayer_marks (id, day, prayer, marked_at, created_at,
                                           updated_at, deleted_at, version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                    day = excluded.day, prayer = excluded.prayer,
                    marked_at = excluded.marked_at, created_at = excluded.created_at,
                    updated_at = excluded.updated_at, deleted_at = excluded.deleted_at,
                    version = excluded.version",
                params![
                    mark.id,
                    mark.day.format("%Y-%m-%d").to_string(),
                    enum_text(&mark.prayer),
                    time_text(mark.marked_at),
                    time_text(mark.created_at),
                    time_text(mark.updated_at),
                    mark.deleted_at.map(time_text),
                    version,
                ],
            )
            .map_err(sqlite_error)?;
        self.append_change("prayer_mark", &mark.id, op, version)?;
        Ok(version)
    }

    pub fn put_mark(&self, mark: &PrayerMark) -> Result<i64, Error> {
        self.write_mark(mark, None)
    }

    pub fn put_mark_versioned(&self, mark: &PrayerMark, version: i64) -> Result<i64, Error> {
        self.write_mark(mark, Some(version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use salah_domain::day::BoundaryId;

    fn t(h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 20, h, 0, 0).unwrap()
    }
    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 20).unwrap()
    }
    fn sample_item() -> Item {
        Item {
            id: "item-1".into(),
            title: "Read Quran".into(),
            block: BoundaryId::Fajr,
            repeat: Repeat::Weekly,
            weekdays: Some(Weekdays::WORKWEEK),
            measure: Measure::Minutes,
            target: Some(20),
            position: 3,
            day: day(),
            created_at: t(1),
            updated_at: t(1),
            deleted_at: None,
            version: 1,
        }
    }
    fn sample_entry() -> Entry {
        Entry {
            id: "entry-1".into(),
            item_id: "item-1".into(),
            block: BlockKey {
                day: day(),
                start: BoundaryId::Fajr,
            },
            kind: EntryKind::Timer,
            value: None,
            started_at: Some(t(2)),
            ended_at: None,
            created_at: t(2),
            updated_at: t(2),
            deleted_at: None,
            version: 1,
        }
    }

    #[test]
    fn rows_round_trip_exactly() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .write(|tx| {
                tx.put_item(&sample_item())?;
                tx.put_entry(&sample_entry())
            })
            .unwrap();
        assert_eq!(store.item("item-1").unwrap(), Some(sample_item()));
        assert_eq!(store.entries().unwrap(), [sample_entry()]);
        assert_eq!(store.running_timers().unwrap().len(), 1);
        assert_eq!(store.entries_between(day(), day()).unwrap().len(), 1);
    }

    #[test]
    fn put_updates_in_place_and_keeps_tombstones() {
        let mut store = Store::open_in_memory().unwrap();
        let mut item = sample_item();
        item.version = store.write(|tx| tx.put_item(&item)).unwrap();
        item.deleted_at = Some(t(3));
        item.updated_at = t(3);
        item.version = store.write(|tx| tx.put_item(&item)).unwrap();
        assert_eq!(store.items().unwrap(), [item]);
    }

    #[test]
    fn prayer_marks_round_trip() {
        let mut store = Store::open_in_memory().unwrap();
        let mark = PrayerMark {
            id: PrayerMark::id_for(day(), BoundaryId::Asr),
            day: day(),
            prayer: BoundaryId::Asr,
            marked_at: t(10),
            created_at: t(10),
            updated_at: t(10),
            deleted_at: None,
            version: 1,
        };
        store.write(|tx| tx.put_mark(&mark)).unwrap();
        assert_eq!(store.mark("2026-11-20:asr").unwrap(), Some(mark.clone()));
        assert_eq!(store.marks_between(day(), day()).unwrap(), [mark]);
    }

    #[test]
    fn a_version_one_database_upgrades_in_place() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute(
            "INSERT INTO items VALUES ('old', 'Old', 'fajr', 'daily', 'check', NULL,
             '2026-11-20', '2026-11-20T01:00:00.000Z', '2026-11-20T01:00:00.000Z', NULL)",
            [],
        )
        .unwrap();
        let store = Store::init(conn).unwrap();
        assert_eq!(store.version().unwrap(), schema_version());
        let old = store.item("old").unwrap().unwrap();
        assert_eq!((old.position, old.weekdays), (0, None));
    }

    #[test]
    fn failed_transactions_save_nothing() {
        let mut store = Store::open_in_memory().unwrap();
        let result: Result<(), Error> = store.write(|tx| {
            tx.put_item(&sample_item())?;
            Err("boom".into())
        });
        assert!(result.is_err());
        assert!(store.items().unwrap().is_empty());
    }

    #[test]
    fn a_version_two_database_with_data_upgrades_in_place() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.execute_batch(MIGRATIONS[1]).unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        conn.execute(
            "INSERT INTO items
             (id, title, block, repeat, measure, target, day, created_at, updated_at,
              deleted_at, position, weekdays)
             VALUES ('old', 'Old', 'fajr', 'weekly', 'minutes', 20, '2026-11-20',
                     '2026-11-20T01:00:00.000Z', '2026-11-20T02:00:00.000Z', NULL, 3, 31)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO entries
             (id, item_id, day, block, kind, value, started_at, ended_at, created_at,
              updated_at, deleted_at)
             VALUES ('entry-old', 'old', '2026-11-20', 'fajr', 'minutes', 20, NULL, NULL,
                     '2026-11-20T01:00:00.000Z', '2026-11-20T01:00:00.000Z', NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO prayer_marks
             (id, day, prayer, marked_at, created_at, updated_at, deleted_at)
             VALUES ('2026-11-20:asr', '2026-11-20', 'asr',
                     '2026-11-20T10:00:00.000Z', '2026-11-20T10:00:00.000Z',
                     '2026-11-20T10:00:00.000Z', NULL)",
            [],
        )
        .unwrap();

        let mut store = Store::init(conn).unwrap();
        assert_eq!(store.version().unwrap(), schema_version());
        let old = store.item("old").unwrap().unwrap();
        assert_eq!(
            (old.position, old.weekdays, old.version),
            (3, Some(Weekdays(31)), 1)
        );
        assert_eq!(store.entry("entry-old").unwrap().unwrap().version, 1);
        assert_eq!(store.mark("2026-11-20:asr").unwrap().unwrap().version, 1);
        assert_eq!(store.revision().unwrap(), 0);
        assert!(store.changes_since(0, 10).unwrap().is_empty());

        let mut old = old;
        old.title = "Updated".into();
        old.updated_at = t(3);
        assert_eq!(store.write(|tx| tx.put_item(&old)).unwrap(), 2);
        assert_eq!(store.revision().unwrap(), 1);
        let changes = store.changes_since(0, 10).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!((changes[0].op.as_str(), changes[0].version), ("update", 2));
    }

    #[test]
    fn one_transaction_bumps_revision_once_and_appends_each_change() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(store.revision().unwrap(), 0);
        let mut first = sample_item();
        first.id = "a".into();
        let mut second = sample_item();
        second.id = "b".into();
        store
            .write(|tx| {
                tx.put_item(&first)?;
                tx.put_item(&second)
            })
            .unwrap();
        assert_eq!(store.revision().unwrap(), 1);
        let changes = store.changes_since(0, 10).unwrap();
        assert_eq!(
            changes
                .iter()
                .map(|change| (
                    change.seq,
                    change.revision,
                    change.op.as_str(),
                    change.version
                ))
                .collect::<Vec<_>>(),
            [(1, 1, "insert", 1), (2, 1, "insert", 1)]
        );
        assert!(changes.iter().all(|change| change.entity == "item"));

        store.write(|_| Ok::<(), Error>(())).unwrap();
        assert_eq!(
            store.revision().unwrap(),
            1,
            "an empty write is not a change"
        );

        first.title = "Changed".into();
        first.updated_at = t(4);
        assert_eq!(store.write(|tx| tx.put_item(&first)).unwrap(), 2);
        second.deleted_at = Some(t(5));
        second.updated_at = t(5);
        assert_eq!(store.write(|tx| tx.put_item(&second)).unwrap(), 2);
        assert_eq!(store.revision().unwrap(), 3);

        let changes = store.changes_since(2, 10).unwrap();
        assert_eq!(
            changes
                .iter()
                .map(|change| (
                    change.revision,
                    change.entity_id.as_str(),
                    change.op.as_str(),
                    change.version
                ))
                .collect::<Vec<_>>(),
            [(2, "a", "update", 2), (3, "b", "delete", 2)]
        );
        assert_eq!(store.changes_since(3, 10).unwrap().len(), 1);
    }

    #[test]
    fn applied_ops_store_results_without_forcing_a_revision() {
        let mut store = Store::open_in_memory().unwrap();
        let item = sample_item();
        store
            .write(|tx| {
                tx.set_op_id(Some("op-1"));
                let version = tx.put_item(&item)?;
                tx.record_applied_op("op-1", r#"{"id":"item-1"}"#)?;
                Ok(version)
            })
            .unwrap();
        let applied = store.applied_op("op-1").unwrap().unwrap();
        assert_eq!(applied.revision, 1);
        assert_eq!(applied.result_json, r#"{"id":"item-1"}"#);
        assert_eq!(
            store.changes_since(0, 10).unwrap()[0].op_id.as_deref(),
            Some("op-1")
        );

        store
            .write(|tx| tx.record_applied_op("op-2", "null"))
            .unwrap();
        assert_eq!(store.revision().unwrap(), 1);
        assert_eq!(store.applied_op("op-2").unwrap().unwrap().revision, 1);
    }

    #[test]
    fn imported_versions_never_move_existing_rows_backwards() {
        let mut store = Store::open_in_memory().unwrap();
        let item = sample_item();
        assert_eq!(store.write(|tx| tx.put_item(&item)).unwrap(), 1);
        let mut imported = item.clone();
        imported.updated_at = t(2);
        assert_eq!(
            store
                .write(|tx| tx.put_item_versioned(&imported, 7))
                .unwrap(),
            7
        );
        assert_eq!(store.item("item-1").unwrap().unwrap().version, 7);
        imported.updated_at = t(3);
        assert_eq!(
            store
                .write(|tx| tx.put_item_versioned(&imported, 2))
                .unwrap(),
            8
        );
    }

    #[test]
    fn reopening_keeps_data_and_refuses_newer_schemas() {
        let dir = std::env::temp_dir().join(format!("salah-store-{}", std::process::id()));
        let path = dir.join("salah.db");
        {
            let mut store = Store::open(&path).unwrap();
            assert_eq!(store.version().unwrap(), schema_version());
            store.write(|tx| tx.put_item(&sample_item())).unwrap();
        }
        assert_eq!(Store::open(&path).unwrap().items().unwrap().len(), 1);
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", schema_version() + 1)
            .unwrap();
        let refusal = Store::open(&path).err().unwrap();
        assert!(refusal.contains("newer"), "{refusal}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
