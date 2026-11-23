use super::STEPS;
use crate::{Store, user_version};
use rusqlite::{Connection, types::Value};
use std::{fs, path::PathBuf};

mod phase3;

struct Database {
    root: PathBuf,
    path: PathBuf,
}

impl Database {
    fn new(test: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("salah-migrations-{}-{test}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self {
            path: root.join("salah.db"),
            root,
        }
    }

    fn at(&self, version: i64, history: bool) -> Connection {
        let conn = Connection::open(&self.path).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        conn.execute_batch(STEPS[0].sql).unwrap();
        conn.execute_batch(include_str!("tests/tracking_v1.sql"))
            .unwrap();
        if history {
            conn.execute_batch(STEPS[1].sql).unwrap();
            let item = conn
                .query_row(
                    "SELECT *, 0 AS metrics_custom FROM items WHERE id='daily'",
                    [],
                    crate::codec::item,
                )
                .unwrap();
            let mut legacy = serde_json::to_value(item).unwrap();
            legacy.as_object_mut().unwrap().remove("metric_ids");
            conn.execute(
                "INSERT INTO item_history VALUES ('daily','2031-05-13',?1)",
                [legacy.to_string()],
            )
            .unwrap();
            conn.execute_batch(
                r#"INSERT INTO config_history VALUES ('2031-05-13','{"sleep_time":"23:00"}');
                INSERT INTO day_clocks VALUES ('2031-05-12', 18000, 330, '2031-05-12T18:00:00Z');"#,
            )
            .unwrap();
        }
        conn.pragma_update(None, "user_version", version).unwrap();
        conn
    }

    fn backups(&self) -> Vec<PathBuf> {
        let mut paths = fs::read_dir(&self.root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("salah.db.bak-v")
                    && !p.extension().is_some_and(|ext| ext == "partial")
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    fn backup(&self, version: i64) -> Connection {
        let paths = self.backups();
        let mut matching = paths.iter().filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&format!("salah.db.bak-v{version}-"))
        });
        let conn = Connection::open(matching.next().unwrap()).unwrap();
        assert!(matching.next().is_none());
        assert_eq!(user_version(&conn).unwrap(), version);
        check_integrity(&conn);
        conn
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn rows(conn: &Connection, query: &str) -> Vec<Vec<Value>> {
    let mut stmt = conn.prepare(query).unwrap();
    let n = stmt.column_count();
    stmt.query_map([], |row| (0..n).map(|i| row.get(i)).collect())
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn check_integrity(conn: &Connection) {
    assert!(rows(conn, "PRAGMA foreign_key_check").is_empty());
    assert_eq!(
        rows(conn, "PRAGMA integrity_check"),
        [[Value::Text("ok".into())]]
    );
}

/// Capture all original columns and rows, including links, sync metadata,
/// replay JSON and sqlite_sequence. Allow for the new rows when crossing 003.
fn original_rows(conn: &Connection) -> Vec<(String, Vec<Vec<Value>>)> {
    let before_v3 = user_version(conn).unwrap() < 3;
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap();
    stmt.query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(|table| {
            let table = table.unwrap();
            let select = conn.prepare(&format!("SELECT * FROM {table}")).unwrap();
            let cols = select.column_names().join(", ");
            let filter = match table.as_str() {
                "metrics" if before_v3 => " WHERE id != 'distractions'",
                "project_metrics" if before_v3 => " WHERE metric_id != 'distractions'",
                _ => "",
            };
            let sql = format!("SELECT {cols} FROM {table}{filter} ORDER BY rowid");
            let values = rows(conn, &sql);
            assert!(!values.is_empty(), "seed {table} before testing an upgrade");
            (sql, values)
        })
        .collect()
}

fn assert_preserved(conn: &Connection, before: &[(String, Vec<Vec<Value>>)]) {
    for (sql, expected) in before {
        assert_eq!(&rows(conn, sql), expected, "{sql}");
    }
    check_integrity(conn);
}

#[test]
fn fresh_database_runs_all_steps_and_skips_the_empty_version_zero_backup() {
    let db = Database::new("fresh");
    let store = Store::open(&db.path).unwrap();
    let v1 = db.backup(1);
    assert!(v1.table_exists(None, "items").unwrap());
    assert!(!v1.table_exists(None, "day_clocks").unwrap());
    assert_eq!(
        rows(&v1, "SELECT id FROM metrics ORDER BY id"),
        [
            [Value::Text("focus".into())],
            [Value::Text("quality".into())]
        ]
    );
    assert!(db.backup(2).table_exists(None, "day_clocks").unwrap());
    assert!(db.backup(3).table_exists(None, "item_metrics").unwrap());
    assert_eq!(store.version().unwrap(), crate::schema_version());
    check_integrity(&store.conn);
    let paths = db.backups();
    assert_eq!(paths.len(), 3);
    drop(Store::open(&db.path).unwrap());
    assert_eq!(db.backups(), paths);
}

#[test]
fn history_step_preserves_each_v1_row_with_or_without_preinstalled_history() {
    for history in [false, true] {
        let db = Database::new(if history { "preinstalled" } else { "plain" });
        let conn = db.at(1, history);
        let before = original_rows(&conn);
        let store = Store::open(&db.path).unwrap();
        let v2 = db.backup(2);
        assert_preserved(&db.backup(1), &before);
        assert_preserved(&v2, &before);
        for table in ["day_clocks", "item_history", "config_history"] {
            assert_eq!(
                rows(&v2, &format!("SELECT * FROM {table}")).len(),
                usize::from(history)
            );
        }
        assert_preserved(&store.conn, &before);
    }
}

#[test]
fn tracking_ui_step_preserves_wal_data_and_adds_only_the_specified_rows() {
    let db = Database::new("wal");
    let seed = db.at(2, true);
    seed.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
        UPDATE sessions SET note='Only in WAL', version=3 WHERE id='work'",
    )
    .unwrap();
    assert!(db.path.with_extension("db-wal").metadata().unwrap().len() > 0);
    let before = original_rows(&seed);
    let store = Store::open(&db.path).unwrap();
    assert_eq!(store.version().unwrap(), crate::schema_version());
    assert_preserved(&db.backup(2), &before);
    let conn = &store.conn;
    assert_preserved(conn, &before);
    assert_eq!(store.metrics().unwrap().len(), 4);
    let metric = store.metric("distractions").unwrap().unwrap();
    assert!(metric.builtin && !metric.archived);
    assert_eq!(
        (metric.name.as_str(), metric.unit.as_str()),
        ("Distractions", "times")
    );
    assert_eq!(metric.kind, salah_domain::tracking::MetricKind::Count);
    assert_eq!(metric.rollup, salah_domain::tracking::Rollup::Sum);
    assert_eq!(metric.scale_max, None);
    let bindings = rows(
        conn,
        "SELECT project_id, required, position FROM project_metrics WHERE metric_id='distractions' ORDER BY project_id",
    );
    assert_eq!(
        bindings,
        [
            vec![
                Value::Text("a".into()),
                Value::Integer(0),
                Value::Integer(0)
            ],
            vec![
                Value::Text("p".into()),
                Value::Integer(0),
                Value::Integer(8)
            ],
        ]
    );
    assert_eq!(
        rows(conn, "SELECT DISTINCT metrics_custom FROM items"),
        [[Value::Integer(0)]]
    );
    assert!(rows(conn, "SELECT * FROM item_metrics").is_empty());
    assert!(rows(conn, "SELECT * FROM session_notes").is_empty());
    assert_eq!(
        rows(
            conn,
            "SELECT name FROM pragma_index_info('sessions_by_item_day') ORDER BY seqno"
        ),
        [[Value::Text("item_id".into())], [Value::Text("day".into())]]
    );
    assert_eq!(
        rows(
            conn,
            "SELECT name FROM pragma_index_info('session_notes_by_session') ORDER BY seqno"
        ),
        [
            [Value::Text("session_id".into())],
            [Value::Text("at".into())]
        ]
    );
}

#[test]
fn failed_step_rolls_back_its_schema_and_version_and_can_be_retried() {
    let db = Database::new("retry");
    let conn = db.at(1, false);
    conn.execute_batch(
        "CREATE TABLE session_notes (existing TEXT); INSERT INTO session_notes VALUES ('keep')",
    )
    .unwrap();
    drop(conn);
    // The first open commits 002 (and keeps v1), then fails partway through 003.
    assert!(Store::open(&db.path).is_err());
    let good_backups = db.backups();
    assert_eq!(good_backups.len(), 1);
    let original_v1 = fs::read(&good_backups[0]).unwrap();
    for _ in 0..4 {
        assert!(Store::open(&db.path).is_err());
        assert_eq!(db.backups(), good_backups);
        assert_eq!(fs::read(&good_backups[0]).unwrap(), original_v1);
    }
    db.backup(1);
    let conn = Connection::open(&db.path).unwrap();
    assert_eq!(user_version(&conn).unwrap(), 2);
    assert!(conn.table_exists(None, "item_history").unwrap());
    assert!(!conn.table_exists(None, "item_metrics").unwrap());
    assert!(rows(&conn, "SELECT id FROM metrics WHERE id='distractions'").is_empty());
    assert_eq!(
        rows(&conn, "SELECT * FROM session_notes"),
        [[Value::Text("keep".into())]]
    );
    assert!(!conn.column_exists(None, "items", "metrics_custom").unwrap());
    conn.execute_batch("ALTER TABLE session_notes RENAME TO saved_notes")
        .unwrap();
    assert_eq!(
        Store::open(&db.path).unwrap().version().unwrap(),
        crate::schema_version()
    );
    check_integrity(&conn);
}

#[test]
fn unknown_or_pretracking_files_are_refused_in_place() {
    for version in [1, crate::schema_version(), crate::schema_version() + 1] {
        let db = Database::new(&format!("unknown-{version}"));
        let conn = Connection::open(&db.path).unwrap();
        conn.execute_batch("CREATE TABLE items(title TEXT); INSERT INTO items VALUES ('keep')")
            .unwrap();
        conn.pragma_update(None, "user_version", version).unwrap();
        let error = Store::open(&db.path).err().unwrap().to_string();
        assert!(error.contains(&format!("version {version}")), "{error}");
        assert!(error.contains("file was left untouched"), "{error}");
        if version <= crate::schema_version() {
            assert!(
                error.contains("pre-tracking") && error.contains("missing days_written"),
                "{error}"
            );
        }
        assert_eq!(fs::read_dir(&db.root).unwrap().count(), 1);
        let reopened = Connection::open(&db.path).unwrap();
        assert_eq!(user_version(&reopened).unwrap(), version);
        assert_eq!(
            rows(&reopened, "SELECT * FROM items"),
            [[Value::Text("keep".into())]]
        );
    }
}

#[test]
fn upgrade_from_v1_rotates_real_backups_and_removes_only_its_own_stale_partials() {
    let db = Database::new("rotation");
    let mut seed = db.at(1, false);
    let oldest = db.root.join("salah.db.bak-v1-10");
    let retained = db.root.join("salah.db.bak-v1-20");
    seed.backup(rusqlite::MAIN_DB, &oldest, None).unwrap();
    seed.backup(rusqlite::MAIN_DB, &retained, None).unwrap();
    let original = fs::read(&retained).unwrap();
    seed.execute("UPDATE meta SET revision=19", []).unwrap();
    let before = original_rows(&seed);
    let stale = db.root.join("salah.db.bak-v2-123.partial");
    fs::write(&stale, "interrupted").unwrap();
    for filename in [
        "other.db.bak-v2-123.partial",
        "salah.db.bak-v2-draft.partial",
    ] {
        fs::write(db.root.join(filename), "unrelated").unwrap();
    }

    // Two successful steps rotate out the oldest of the two preexisting copies.
    super::run(&mut seed, Some(&db.path), &STEPS[..3]).unwrap();
    assert_eq!(user_version(&seed).unwrap(), 3);
    assert!(!oldest.exists());
    assert!(!stale.exists());
    assert_eq!(fs::read(&retained).unwrap(), original);
    for filename in [
        "other.db.bak-v2-123.partial",
        "salah.db.bak-v2-draft.partial",
    ] {
        assert_eq!(
            fs::read_to_string(db.root.join(filename)).unwrap(),
            "unrelated"
        );
    }
    let backups = db.backups();
    assert_eq!(backups.len(), 3);
    let mut versions = Vec::new();
    for path in backups {
        let conn = Connection::open(&path).unwrap();
        versions.push(user_version(&conn).unwrap());
        if path != retained {
            assert_preserved(&conn, &before);
        } else {
            check_integrity(&conn);
        }
    }
    assert_eq!(versions, [1, 1, 2]);
    assert_preserved(&seed, &before);
}

#[test]
fn unavailable_backup_directory_leaves_the_source_at_its_original_version() {
    let db = Database::new("backup-error");
    let mut conn = db.at(2, true);
    let before = original_rows(&conn);
    let unavailable = db.root.join("missing/salah.db");
    assert!(super::apply(&mut conn, Some(&unavailable)).is_err());
    assert_eq!(user_version(&conn).unwrap(), 2);
    assert!(!conn.table_exists(None, "item_metrics").unwrap());
    assert_preserved(&conn, &before);
}

#[test]
fn tracking_ui_constraints_protect_task_selections_and_session_notes() {
    let db = Database::new("constraints");
    drop(db.at(2, true));
    let store = Store::open(&db.path).unwrap();
    let conn = &store.conn;
    assert!(
        conn.execute("UPDATE items SET metrics_custom=2", [])
            .is_err()
    );
    conn.execute("INSERT INTO item_metrics VALUES ('daily','focus',0)", [])
        .unwrap();
    for (item, metric) in [
        ("daily", "focus"),
        ("missing", "focus"),
        ("daily", "missing"),
    ] {
        assert!(
            conn.execute("INSERT INTO item_metrics VALUES (?1,?2,1)", [item, metric])
                .is_err()
        );
    }

    let insert = "INSERT INTO session_notes VALUES ('n',?1,'2031-05-12T03:05:00Z',?2,
        '2031-05-12T03:05:00Z','2031-05-12T03:05:00Z',NULL,?3)";
    for (session, text, version) in [
        ("work", "", 1),
        ("work", &"x".repeat(501), 1),
        ("missing", "ok", 1),
        ("work", "ok", 0),
    ] {
        assert!(
            conn.execute(insert, rusqlite::params![session, text, version])
                .is_err()
        );
    }
    conn.execute(insert, rusqlite::params!["work", "x".repeat(500), 1])
        .unwrap();
    conn.execute("DELETE FROM sessions WHERE id='work'", [])
        .unwrap();
    assert!(rows(conn, "SELECT * FROM session_notes").is_empty());
    check_integrity(conn);
}

#[test]
fn failed_commit_discards_its_backup_and_keeps_older_copies() {
    let db = Database::new("commit-busy");
    let reader = db.at(2, true);
    for time in [10, 20, 30] {
        reader
            .backup(
                rusqlite::MAIN_DB,
                db.root.join(format!("salah.db.bak-v2-{time}")),
                None,
            )
            .unwrap();
    }
    let before = original_rows(&reader);
    let backups = db.backups();
    let mut writer = Connection::open(&db.path).unwrap();
    writer.busy_timeout(std::time::Duration::ZERO).unwrap();
    // In rollback-journal mode this reader permits BEGIN IMMEDIATE and the
    // migration writes, but prevents promotion to the exclusive commit lock.
    reader.execute_batch("BEGIN; SELECT * FROM items").unwrap();
    let error = super::apply(&mut writer, Some(&db.path)).unwrap_err();
    assert_eq!(error.code(), "busy");
    assert!(writer.is_autocommit());
    assert_eq!(db.backups(), backups);
    assert_eq!(user_version(&writer).unwrap(), 2);
    assert_preserved(&writer, &before);
    reader.execute_batch("COMMIT").unwrap();
    super::apply(&mut writer, Some(&db.path)).unwrap();
    assert_eq!(user_version(&writer).unwrap(), crate::schema_version());
    assert!(!backups[0].exists());
}

#[test]
fn rebuild_step_checks_references_before_commit_and_restores_enforcement_on_every_exit() {
    let db = Database::new("rebuild");
    drop(db.at(2, true));
    let mut store = Store::open(&db.path).unwrap();
    let backups = db.backups();
    let run_step = |conn: &mut Connection, sql| {
        let mut steps = STEPS.to_vec();
        steps.push(super::Step {
            sql,
            foreign_keys_off: true,
        });
        super::run(conn, Some(&db.path), &steps)
    };
    for (sql, expected) in [
        (
            "DELETE FROM days_written",
            "violates a foreign key in day_clocks",
        ),
        (
            "DROP TABLE days_written; SELECT * FROM nonexistent",
            "nonexistent",
        ),
    ] {
        let error = run_step(&mut store.conn, sql).unwrap_err();
        assert!(error.contains(expected), "{error}");
        assert_eq!(store.version().unwrap(), crate::schema_version());
        assert!(store.foreign_keys_enabled().unwrap());
        assert_eq!(db.backups(), backups);
        assert_eq!(
            rows(&store.conn, "SELECT day FROM days_written"),
            [[Value::Text("2031-05-12".into())]]
        );
        check_integrity(&store.conn);
    }

    run_step(
        &mut store.conn,
        "CREATE TABLE rebuilt(day TEXT PRIMARY KEY, written_at TEXT NOT NULL);
        INSERT INTO rebuilt SELECT * FROM days_written;
        DROP TABLE days_written;
        ALTER TABLE rebuilt RENAME TO days_written;",
    )
    .unwrap();
    assert_eq!(store.version().unwrap(), crate::schema_version() + 1);
    assert!(store.foreign_keys_enabled().unwrap());
    assert!(store.conn.execute("DELETE FROM days_written", []).is_err());
    assert_eq!(
        rows(
            &store.conn,
            "SELECT day FROM day_clocks JOIN days_written USING(day)"
        ),
        [[Value::Text("2031-05-12".into())]]
    );
    db.backup(crate::schema_version());
    check_integrity(&store.conn);
}
