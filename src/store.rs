//! Local SQLite persistence for items and entries.
//!
//! The store only saves and loads rows; planning rules live in `item` and
//! `planner`. Rows are never physically deleted: `deleted_at` tombstones let
//! deletions travel through export/import and, later, sync.

use crate::error::Error;
use crate::item::{Entry, EntryKind, Item, Measure, PrayerMark, Repeat, Weekdays};
use crate::schedule::BlockKey;
use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Serialize, de::DeserializeOwned};
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
    })
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
        let value = match work(&WriteTx { store: self }) {
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
pub struct WriteTx<'a> {
    store: &'a Store,
}

impl Deref for WriteTx<'_> {
    type Target = Store;

    fn deref(&self) -> &Store {
        self.store
    }
}

impl WriteTx<'_> {
    pub fn put_item(&self, item: &Item) -> Result<(), Error> {
        self.store
            .conn
            .execute(
                "INSERT INTO items (id, title, block, repeat, measure, target, day,
                                    created_at, updated_at, deleted_at, position, weekdays)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title, block = excluded.block,
                    repeat = excluded.repeat, measure = excluded.measure,
                    target = excluded.target, day = excluded.day,
                    position = excluded.position, weekdays = excluded.weekdays,
                    created_at = excluded.created_at, updated_at = excluded.updated_at,
                    deleted_at = excluded.deleted_at",
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
                ],
            )
            .map(|_| ())
            .map_err(sqlite_error)
    }

    pub fn put_entry(&self, entry: &Entry) -> Result<(), Error> {
        self.store
            .conn
            .execute(
                "INSERT INTO entries (id, item_id, day, block, kind, value, started_at,
                                      ended_at, created_at, updated_at, deleted_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(id) DO UPDATE SET
                    item_id = excluded.item_id, day = excluded.day,
                    block = excluded.block, kind = excluded.kind,
                    value = excluded.value, started_at = excluded.started_at,
                    ended_at = excluded.ended_at, created_at = excluded.created_at,
                    updated_at = excluded.updated_at, deleted_at = excluded.deleted_at",
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
                ],
            )
            .map(|_| ())
            .map_err(sqlite_error)
    }

    pub fn put_mark(&self, mark: &PrayerMark) -> Result<(), Error> {
        self.store
            .conn
            .execute(
                "INSERT INTO prayer_marks (id, day, prayer, marked_at, created_at,
                                           updated_at, deleted_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                    day = excluded.day, prayer = excluded.prayer,
                    marked_at = excluded.marked_at, created_at = excluded.created_at,
                    updated_at = excluded.updated_at, deleted_at = excluded.deleted_at",
                params![
                    mark.id,
                    mark.day.format("%Y-%m-%d").to_string(),
                    enum_text(&mark.prayer),
                    time_text(mark.marked_at),
                    time_text(mark.created_at),
                    time_text(mark.updated_at),
                    mark.deleted_at.map(time_text),
                ],
            )
            .map(|_| ())
            .map_err(sqlite_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::day::BoundaryId;
    use chrono::TimeZone;

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
        store.write(|tx| tx.put_item(&item)).unwrap();
        item.deleted_at = Some(t(3));
        item.updated_at = t(3);
        store.write(|tx| tx.put_item(&item)).unwrap();
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
