//! Local SQLite persistence for items and entries.
//!
//! The store only saves and loads rows; planning rules live in `item` and
//! `planner`. Rows are never physically deleted: `deleted_at` tombstones let
//! deletions travel through export/import and, later, sync.

use crate::item::{Entry, EntryKind, Item, Measure, Repeat};
use crate::schedule::BlockKey;
use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Serialize, de::DeserializeOwned};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Each entry upgrades the schema by one version. Never edit a released
/// migration; append a new one.
const MIGRATIONS: &[&str] = &["
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
"];

pub fn schema_version() -> i64 {
    MIGRATIONS.len() as i64
}

/// `${XDG_DATA_HOME:-~/.local/share}/salah/salah.db`
pub fn default_path() -> PathBuf {
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

fn err(e: impl std::fmt::Display) -> String {
    format!("store: {e}")
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
        measure: parse_enum::<Measure>(&row.get::<_, String>("measure")?)?,
        target: row.get("target")?,
        day: parse_day(&row.get::<_, String>("day")?)?,
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
    pub fn open(path: &Path) -> Result<Store, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(err)?;
        }
        let conn = Connection::open(path).map_err(err)?;
        // WAL lets the widget read while the app or CLI writes.
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(err)?;
        Store::init(conn)
    }

    pub fn open_in_memory() -> Result<Store, String> {
        Store::init(Connection::open_in_memory().map_err(err)?)
    }

    fn init(conn: Connection) -> Result<Store, String> {
        conn.busy_timeout(Duration::from_secs(3)).map_err(err)?;
        let mut store = Store { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn version(&self) -> Result<i64, String> {
        self.conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(err)
    }

    fn migrate(&mut self) -> Result<(), String> {
        let current = self.version()?;
        if current > schema_version() {
            return Err(format!(
                "store: database schema {current} is newer than this salah ({}); update salah",
                schema_version()
            ));
        }
        for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
            let tx = self.conn.transaction().map_err(err)?;
            tx.execute_batch(sql).map_err(err)?;
            tx.pragma_update(None, "user_version", index as i64 + 1)
                .map_err(err)?;
            tx.commit().map_err(err)?;
        }
        Ok(())
    }

    /// Runs `work` in one transaction; nothing is saved if it fails.
    pub fn transaction<T>(
        &mut self,
        work: impl FnOnce(&Store) -> Result<T, String>,
    ) -> Result<T, String> {
        self.conn.execute_batch("BEGIN IMMEDIATE").map_err(err)?;
        match work(self) {
            Ok(value) => {
                self.conn.execute_batch("COMMIT").map_err(err)?;
                Ok(value)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    pub fn put_item(&self, item: &Item) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO items (id, title, block, repeat, measure, target, day,
                                    created_at, updated_at, deleted_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title, block = excluded.block,
                    repeat = excluded.repeat, measure = excluded.measure,
                    target = excluded.target, day = excluded.day,
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
                ],
            )
            .map(|_| ())
            .map_err(err)
    }

    pub fn put_entry(&self, entry: &Entry) -> Result<(), String> {
        self.conn
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
            .map_err(err)
    }

    pub fn item(&self, id: &str) -> Result<Option<Item>, String> {
        self.conn
            .query_row("SELECT * FROM items WHERE id = ?1", [id], item_from)
            .optional()
            .map_err(err)
    }

    pub fn entry(&self, id: &str) -> Result<Option<Entry>, String> {
        self.conn
            .query_row("SELECT * FROM entries WHERE id = ?1", [id], entry_from)
            .optional()
            .map_err(err)
    }

    /// Every item, deleted ones included, oldest first.
    pub fn items(&self) -> Result<Vec<Item>, String> {
        self.query("SELECT * FROM items ORDER BY created_at, id", [], item_from)
    }

    /// Every entry, deleted ones included, oldest first.
    pub fn entries(&self) -> Result<Vec<Entry>, String> {
        self.query(
            "SELECT * FROM entries ORDER BY created_at, id",
            [],
            entry_from,
        )
    }

    /// Entries whose block lies in the inclusive day range.
    pub fn entries_between(&self, first: NaiveDate, last: NaiveDate) -> Result<Vec<Entry>, String> {
        self.query(
            "SELECT * FROM entries WHERE day BETWEEN ?1 AND ?2 ORDER BY created_at, id",
            [
                first.format("%Y-%m-%d").to_string(),
                last.format("%Y-%m-%d").to_string(),
            ],
            entry_from,
        )
    }

    pub fn running_timers(&self) -> Result<Vec<Entry>, String> {
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
    ) -> Result<Vec<T>, String> {
        let mut statement = self.conn.prepare(sql).map_err(err)?;
        statement
            .query_map(params, map)
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<T>>>()
            .map_err(err)
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
            repeat: Repeat::Daily,
            measure: Measure::Minutes,
            target: Some(20),
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
        let store = Store::open_in_memory().unwrap();
        store.put_item(&sample_item()).unwrap();
        store.put_entry(&sample_entry()).unwrap();
        assert_eq!(store.item("item-1").unwrap(), Some(sample_item()));
        assert_eq!(store.entries().unwrap(), [sample_entry()]);
        assert_eq!(store.running_timers().unwrap().len(), 1);
        assert_eq!(store.entries_between(day(), day()).unwrap().len(), 1);
    }

    #[test]
    fn put_updates_in_place_and_keeps_tombstones() {
        let store = Store::open_in_memory().unwrap();
        let mut item = sample_item();
        store.put_item(&item).unwrap();
        item.deleted_at = Some(t(3));
        item.updated_at = t(3);
        store.put_item(&item).unwrap();
        assert_eq!(store.items().unwrap(), [item]);
    }

    #[test]
    fn failed_transactions_save_nothing() {
        let mut store = Store::open_in_memory().unwrap();
        let result: Result<(), String> = store.transaction(|s| {
            s.put_item(&sample_item())?;
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
            let store = Store::open(&path).unwrap();
            assert_eq!(store.version().unwrap(), schema_version());
            store.put_item(&sample_item()).unwrap();
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
