use super::phase3::populated_v3;
use super::{Database, STEPS, assert_preserved, check_integrity, original_rows, rows};
use crate::{Store, user_version};
use rusqlite::{Connection, ErrorCode, types::Value};

/// A realistic v4 file without any backup copies yet.
fn populated_v4(db: &Database) -> Connection {
    let mut conn = populated_v3(db);
    crate::migrations::run(&mut conn, None, &STEPS[..4]).unwrap();
    conn.execute_batch(include_str!("tracking_v4.sql")).unwrap();
    assert_eq!(user_version(&conn).unwrap(), 4);
    check_integrity(&conn);
    conn
}

fn text(value: &str) -> Value {
    Value::Text(value.into())
}

fn pairs(values: &[(&str, &str)]) -> Vec<Vec<Value>> {
    values.iter().map(|(a, b)| vec![text(a), text(b)]).collect()
}

fn counts(conn: &Connection, tables: &[String]) -> Vec<(String, i64)> {
    tables
        .iter()
        .map(|table| {
            let n = conn
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            (table.clone(), n)
        })
        .collect()
}

fn tables(conn: &Connection) -> Vec<String> {
    rows(
        conn,
        "SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name",
    )
    .into_iter()
    .map(|row| match &row[0] {
        Value::Text(name) => name.clone(),
        other => panic!("{other:?}"),
    })
    .collect()
}

/// The sessions statistics read: live and not running (ticks included).
const LIVE: &str = "SELECT * FROM sessions WHERE deleted_at IS NULL
    AND (started_at IS NULL OR ended_at IS NOT NULL)";
/// `stats.table` group "tag" before 005: each live tag of the session's project.
const PROJECT_RULE: &str = "SELECT s.id, t.id FROM live s
    JOIN project_tags pt ON pt.project_id = s.project_id
    JOIN tags t ON t.id = pt.tag_id AND t.deleted_at IS NULL";
/// data-model.md 005: its task's live tags, or its project's for work without a task.
const TASK_RULE: &str = "SELECT s.id, t.id FROM live s
    JOIN item_tags it ON it.item_id = s.item_id
    JOIN tags t ON t.id = it.tag_id AND t.deleted_at IS NULL
    UNION ALL
    SELECT s.id, t.id FROM live s
    JOIN project_tags pt ON pt.project_id = s.project_id
    JOIN tags t ON t.id = pt.tag_id AND t.deleted_at IS NULL
    WHERE s.item_id IS NULL";

/// Each session with each of its tag keys ("none" for "No tag").
fn session_tags(conn: &Connection, rule: &str) -> Vec<Vec<Value>> {
    rows(
        conn,
        &format!(
            "WITH live AS ({LIVE}), tagged(session, tag) AS ({rule})
             SELECT l.id, COALESCE(tagged.tag, 'none') FROM live l
             LEFT JOIN tagged ON tagged.session = l.id ORDER BY 1, 2"
        ),
    )
}

/// The "tag" group rows: key, sessions, minutes (a session counts once per tag).
fn tag_totals(conn: &Connection, rule: &str) -> Vec<Vec<Value>> {
    rows(
        conn,
        &format!(
            "WITH live AS ({LIVE}), tagged(session, tag) AS ({rule}),
             keyed AS (SELECT l.minutes, COALESCE(tagged.tag, 'none') AS key FROM live l
                       LEFT JOIN tagged ON tagged.session = l.id)
             SELECT key, count(*), COALESCE(sum(minutes), 0) FROM keyed GROUP BY key ORDER BY key"
        ),
    )
}

fn total(key: &str, sessions: i64, minutes: i64) -> Vec<Value> {
    vec![text(key), Value::Integer(sessions), Value::Integer(minutes)]
}

#[test]
fn v5_preserves_every_v4_row_and_gives_each_task_its_project_tags() {
    let db = Database::new("phase4");
    let seed = populated_v4(&db);
    let before = original_rows(&seed);
    let v4_tables = tables(&seed);
    let v4_counts = counts(&seed, &v4_tables);
    let v4_session_tags = session_tags(&seed, PROJECT_RULE);
    let v4_totals = tag_totals(&seed, PROJECT_RULE);
    drop(seed);

    let store = Store::open(&db.path).unwrap();
    let conn = &store.conn;
    assert_eq!(store.version().unwrap(), 5);
    assert_eq!(crate::schema_version(), 5);
    assert!(store.foreign_keys_enabled().unwrap());
    assert_eq!(db.backups().len(), 1);
    let backup = db.backup(4);
    assert_preserved(&backup, &before);
    assert_eq!(tables(&backup), v4_tables);
    assert_preserved(conn, &before);
    assert_eq!(counts(conn, &v4_tables), v4_counts);
    let mut expected_tables = v4_tables.clone();
    expected_tables.extend(["goals", "item_tags", "reviews"].map(String::from));
    expected_tables.sort();
    assert_eq!(tables(conn), expected_tables);
    for table in ["goals", "reviews"] {
        assert!(rows(conn, &format!("SELECT * FROM {table}")).is_empty());
    }

    // Tagged projects' tasks, removed ones included, carry exactly the project's live tags
    // (not the link to the removed tag 'x'); tasks of untagged projects or none get nothing.
    assert_eq!(
        rows(conn, "SELECT item_id, tag_id FROM item_tags ORDER BY 1, 2"),
        pairs(&[
            ("backlog", "t"),
            ("backlog", "u"),
            ("course", "t"),
            ("course", "u"),
            ("daily", "t"),
            ("dropped", "t"),
            ("dropped", "u"),
            ("once", "t"),
        ])
    );
    const LIVE_PROJECT_TAGS: &str = "SELECT i.id, pt.tag_id FROM items i
        JOIN project_tags pt ON pt.project_id = i.project_id
        JOIN tags t ON t.id = pt.tag_id AND t.deleted_at IS NULL";
    for sql in [
        format!("{LIVE_PROJECT_TAGS} EXCEPT SELECT item_id, tag_id FROM item_tags"),
        format!("SELECT item_id, tag_id FROM item_tags EXCEPT {LIVE_PROJECT_TAGS}"),
    ] {
        assert!(rows(conn, &sql).is_empty(), "{sql}");
    }
    assert_eq!(
        rows(conn, "SELECT project_id FROM project_tags WHERE tag_id='x'"),
        [[text("q")]]
    );
    assert!(rows(conn, "SELECT * FROM item_tags WHERE tag_id='x'").is_empty());
    assert_eq!(
        rows(
            conn,
            "SELECT i.id FROM items i WHERE NOT EXISTS
             (SELECT 1 FROM item_tags it WHERE it.item_id = i.id) ORDER BY 1"
        ),
        [[text("errand")], [text("removed")], [text("weekly")]]
    );

    // Statistics by tag read the same on day one.
    assert_eq!(session_tags(conn, TASK_RULE), v4_session_tags);
    assert_eq!(tag_totals(conn, TASK_RULE), v4_totals);
    assert_eq!(
        v4_totals,
        [total("none", 2, 35), total("t", 7, 152), total("u", 4, 115)]
    );

    for (child, column, parent) in [
        ("goals", "project_id", "projects"),
        ("goals", "epic_id", "epics"),
        ("goals", "item_id", "items"),
        ("goals", "metric_id", "metrics"),
        ("goals", "replaces_id", "goals"),
        ("item_tags", "item_id", "items"),
        ("item_tags", "tag_id", "tags"),
    ] {
        let references: i64 = conn
            .query_row(
                r#"SELECT count(*) FROM pragma_foreign_key_list(?1)
               WHERE "from"=?2 AND "table"=?3 AND "to"='id'"#,
                [child, column, parent],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(references, 1, "{child}.{column} references {parent}.id");
    }
    for (table, index, columns, unique, partial) in [
        ("goals", "goals_active", "ends_on,deleted_at", 0, 0),
        ("reviews", "reviews_by_week", "week", 1, 1),
        ("item_tags", "item_tags_by_tag", "tag_id", 0, 0),
    ] {
        assert_eq!(
            rows(
                conn,
                &format!(
                    "SELECT group_concat(name, ',' ORDER BY seqno) FROM pragma_index_info('{index}')"
                )
            ),
            [[text(columns)]],
            "{index}"
        );
        assert_eq!(
            rows(
                conn,
                &format!(
                    "SELECT \"unique\", partial FROM pragma_index_list('{table}') WHERE name='{index}'"
                )
            ),
            [[Value::Integer(unique), Value::Integer(partial)]],
            "{index}"
        );
    }
    assert_eq!(
        rows(
            conn,
            "SELECT group_concat(name, ',' ORDER BY seqno) FROM pragma_index_info(
             (SELECT name FROM pragma_index_list('item_tags') WHERE origin='pk'))"
        ),
        [[text("item_id,tag_id")]]
    );
}

#[test]
fn v5_constraints_reject_bad_goals_reviews_and_task_tags() {
    let db = Database::new("phase4-constraints");
    drop(populated_v4(&db));
    let store = Store::open(&db.path).unwrap();
    let conn = &store.conn;
    let goal = "INSERT INTO goals (id, project_id, epic_id, item_id, period, metric_id, direction,
        target, starts_on, ends_on, replaces_id, position, created_at, updated_at, version) VALUES";
    conn.execute_batch(&format!(
        "{goal} ('all', NULL, NULL, NULL, 'week', NULL, 'at_least', 600, '2031-05-12', NULL, NULL, 0,
                 '2031-05-12', '2031-05-12', 1),
                ('distract', 'q', NULL, NULL, 'day', 'distractions', 'at_most', 3, '2031-05-12',
                 '2031-05-12', NULL, 1, '2031-05-12', '2031-05-12', 1),
                ('epic', NULL, 'e', NULL, 'day', 'focus', 'at_least', 3.5, '2031-05-13', NULL,
                 'distract', 2, '2031-05-13', '2031-05-13', 1),
                ('task', NULL, NULL, 'course', 'week', NULL, 'at_least', 0.5, '2031-05-12', NULL,
                 NULL, 3, '2031-05-12', '2031-05-12', 1);
         INSERT INTO goals (id, period, target, starts_on, position, created_at, updated_at, version)
             VALUES ('defaults', 'day', 30, '2031-05-12', 4, '2031-05-12', '2031-05-12', 1);
         INSERT INTO reviews (id, week, note, created_at, updated_at, version)
             VALUES ('w1', '2031-05-12', 'Good week', '2031-05-18', '2031-05-18', 1);
         INSERT INTO item_tags VALUES ('weekly', 'u');"
    ))
    .unwrap();
    assert_eq!(
        rows(conn, "SELECT direction FROM goals WHERE id='defaults'"),
        [[text("at_least")]]
    );

    for sql in [
        // At most one scope id.
        "UPDATE goals SET epic_id='e' WHERE id='distract'",
        "UPDATE goals SET item_id='course' WHERE id='distract'",
        "UPDATE goals SET item_id='course' WHERE id='epic'",
        "UPDATE goals SET project_id='q', epic_id='e', item_id='course' WHERE id='all'",
        // at_most only on a metric.
        "UPDATE goals SET direction='at_most' WHERE id='all'",
        "UPDATE goals SET metric_id=NULL WHERE id='distract'",
        "UPDATE goals SET direction='below' WHERE id='distract'",
        "UPDATE goals SET direction=NULL WHERE id='distract'",
        // Periods and dates.
        "UPDATE goals SET period='month' WHERE id='all'",
        "UPDATE goals SET period=NULL WHERE id='all'",
        "UPDATE goals SET ends_on='2031-05-11' WHERE id='all'",
        "UPDATE goals SET starts_on='2031-05-13' WHERE id='distract'",
        "UPDATE goals SET starts_on=NULL WHERE id='all'",
        // A positive target.
        "UPDATE goals SET target=0 WHERE id='all'",
        "UPDATE goals SET target=-5 WHERE id='all'",
        "UPDATE goals SET target=NULL WHERE id='all'",
        // Sync columns.
        "UPDATE goals SET version=0 WHERE id='all'",
        "UPDATE goals SET position=NULL WHERE id='all'",
        "UPDATE goals SET created_at=NULL WHERE id='all'",
        // References.
        "UPDATE goals SET project_id='missing' WHERE id='distract'",
        "UPDATE goals SET epic_id='missing' WHERE id='epic'",
        "UPDATE goals SET item_id='missing' WHERE id='task'",
        "UPDATE goals SET metric_id='missing' WHERE id='distract'",
        "UPDATE goals SET replaces_id='missing' WHERE id='epic'",
        "DELETE FROM goals WHERE id='distract'",
        // One live review per week, with a bounded note.
        "INSERT INTO reviews (id, week, note, created_at, updated_at, version)
         VALUES ('w2', '2031-05-12', 'Again', '2031-05-18', '2031-05-18', 1)",
        "UPDATE reviews SET note='' WHERE id='w1'",
        "UPDATE reviews SET note=NULL WHERE id='w1'",
        "UPDATE reviews SET note=printf('%.5001c', 'x') WHERE id='w1'",
        "UPDATE reviews SET week=NULL WHERE id='w1'",
        "UPDATE reviews SET version=0 WHERE id='w1'",
        // Task tags: one row per pair, both ends real.
        "INSERT INTO item_tags VALUES ('once', 't')",
        "INSERT INTO item_tags VALUES ('missing', 't')",
        "INSERT INTO item_tags VALUES ('once', 'missing')",
        "INSERT INTO item_tags VALUES ('once', NULL)",
        "DELETE FROM tags WHERE id='u'",
        "DELETE FROM items WHERE id='weekly'",
    ] {
        let err = conn.execute(sql, []).expect_err(sql);
        assert_eq!(
            err.sqlite_error_code(),
            Some(ErrorCode::ConstraintViolation),
            "{sql}"
        );
    }

    // The edges that must still be accepted.
    conn.execute_batch(
        "UPDATE goals SET ends_on=starts_on WHERE id='all';
         UPDATE goals SET ends_on=NULL WHERE id='all';
         UPDATE reviews SET note=printf('%.5000c', 'x') WHERE id='w1';
         UPDATE reviews SET deleted_at='2031-05-19' WHERE id='w1';
         INSERT INTO reviews (id, week, note, created_at, updated_at, version)
             VALUES ('w2', '2031-05-12', 'Rewritten', '2031-05-19', '2031-05-19', 1);
         UPDATE reviews SET deleted_at='2031-05-20' WHERE id='w2';
         INSERT INTO reviews (id, week, note, created_at, updated_at, version)
             VALUES ('w3', '2031-05-12', 'Third', '2031-05-20', '2031-05-20', 1);",
    )
    .unwrap();
    let err = conn
        .execute("UPDATE reviews SET deleted_at=NULL WHERE id='w1'", [])
        .unwrap_err();
    assert_eq!(
        err.sqlite_error_code(),
        Some(ErrorCode::ConstraintViolation)
    );
    assert_eq!(
        rows(
            conn,
            "SELECT id FROM reviews WHERE week='2031-05-12' AND deleted_at IS NULL"
        ),
        [[text("w3")]]
    );
    check_integrity(conn);
}

#[test]
fn v5_is_a_no_op_when_the_runner_meets_it_again() {
    let db = Database::new("phase4-rerun");
    drop(populated_v4(&db));
    let mut store = Store::open(&db.path).unwrap();
    store
        .conn
        .execute_batch(
            "INSERT INTO goals (id, period, target, starts_on, position, created_at, updated_at, version)
                 VALUES ('g', 'week', 300, '2031-05-12', 0, '2031-05-12', '2031-05-12', 1);
             INSERT INTO reviews (id, week, note, created_at, updated_at, version)
                 VALUES ('w', '2031-05-12', 'Steady', '2031-05-18', '2031-05-18', 1);
             DELETE FROM item_tags WHERE item_id='daily';",
        )
        .unwrap();
    let after = original_rows(&store.conn);
    let backups = db.backups();
    assert_eq!(backups.len(), 1);

    // The runner, a reopen, and the step list cut at 005 all leave the file alone:
    // no second copy of project tags (the 'daily' row stays deleted), no new backup.
    crate::migrations::apply(&mut store.conn, Some(&db.path)).unwrap();
    crate::migrations::run(&mut store.conn, Some(&db.path), &STEPS[..5]).unwrap();
    drop(store);
    let store = Store::open(&db.path).unwrap();
    assert_eq!(store.version().unwrap(), 5);
    assert_preserved(&store.conn, &after);
    assert_eq!(db.backups(), backups);
    assert!(rows(&store.conn, "SELECT * FROM item_tags WHERE item_id='daily'").is_empty());
}

#[test]
fn v5_failure_rolls_back_and_discards_only_its_own_backup() {
    let db = Database::new("phase4-rollback");
    let mut conn = populated_v4(&db);
    for time in [10, 20, 30] {
        let path = db.root.join(format!("salah.db.bak-v3-{time}"));
        conn.backup(rusqlite::MAIN_DB, path, None).unwrap();
    }
    // A stray index named like 005's last one fails the step after its tables exist.
    conn.execute_batch("CREATE INDEX item_tags_by_tag ON tags (name)")
        .unwrap();
    let before = original_rows(&conn);
    let older = db.backups();

    let error = crate::migrations::apply(&mut conn, Some(&db.path)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("index item_tags_by_tag already exists"),
        "{error}"
    );
    assert_eq!(user_version(&conn).unwrap(), 4);
    assert_eq!(rows(&conn, "PRAGMA foreign_keys"), [[Value::Integer(1)]]);
    assert_eq!(db.backups(), older);
    for table in ["goals", "reviews", "item_tags"] {
        assert!(!conn.table_exists(None, table).unwrap(), "{table}");
    }
    assert_eq!(original_rows(&conn), before);

    conn.execute_batch("DROP INDEX item_tags_by_tag").unwrap();
    let repaired = original_rows(&conn);
    crate::migrations::apply(&mut conn, Some(&db.path)).unwrap();
    assert_eq!(user_version(&conn).unwrap(), 5);
    assert_preserved(&conn, &repaired);
    // Keep three: the oldest v3 copy rotates out for the new v4 one.
    let backups = db.backups();
    assert_eq!(backups.len(), 3);
    assert!(!older[0].exists());
    assert_eq!(backups[..2], older[1..]);
    assert_preserved(&db.backup(4), &repaired);
}

#[test]
fn full_upgrade_from_v1_keeps_exactly_the_v2_v3_and_v4_backups() {
    let db = Database::new("phase4-rotation");
    let mut seed = db.at(1, false);
    let older = ["salah.db.bak-v1-10", "salah.db.bak-v1-20"].map(|name| db.root.join(name));
    for path in &older {
        seed.backup(rusqlite::MAIN_DB, path, None).unwrap();
    }
    let before = original_rows(&seed);

    crate::migrations::apply(&mut seed, Some(&db.path)).unwrap();
    assert_eq!(user_version(&seed).unwrap(), crate::schema_version());
    assert_preserved(&seed, &before);
    for path in &older {
        assert!(!path.exists(), "{} was not rotated out", path.display());
    }
    let versions: Vec<i64> = db
        .backups()
        .iter()
        .map(|path| user_version(&Connection::open(path).unwrap()).unwrap())
        .collect();
    assert_eq!(versions, [2, 3, 4]);
    // Each copy is the database as it stood before its own step.
    assert_preserved(&db.backup(2), &before);
    assert_preserved(&db.backup(3), &before);
    let v4 = db.backup(4);
    assert_preserved(&v4, &before);
    assert!(v4.table_exists(None, "epics").unwrap());
    assert!(!v4.table_exists(None, "item_tags").unwrap());
    assert_eq!(
        rows(&seed, "SELECT item_id, tag_id FROM item_tags ORDER BY 1, 2"),
        pairs(&[("daily", "t"), ("once", "t")])
    );
}

/// A session keeps the project it was written with, but a task carries the tags of
/// the project it is in at migration time. Work logged before the task moved is
/// therefore relabelled by 005: the one case where "tag" totals can change.
#[test]
fn v5_labels_work_of_a_moved_task_with_its_current_project_tags() {
    let db = Database::new("phase4-moved");
    let seed = populated_v4(&db);
    seed.execute_batch(
        "UPDATE items SET project_id='n', epic_id=NULL, metrics_custom=0, version=2 WHERE id='course';
         DELETE FROM item_metrics WHERE item_id='course';",
    )
    .unwrap();
    let mut expected = session_tags(&seed, PROJECT_RULE);
    for row in &mut expected {
        if matches!(&row[0], Value::Text(id) if id.starts_with("lecture")) {
            row[1] = text("none");
        }
    }
    expected.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    expected.dedup();
    drop(seed);

    let store = Store::open(&db.path).unwrap();
    let mut after = session_tags(&store.conn, TASK_RULE);
    after.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    assert_eq!(after, expected);
    assert_eq!(
        tag_totals(&store.conn, TASK_RULE),
        [total("none", 4, 85), total("t", 5, 102), total("u", 2, 65)]
    );
}
