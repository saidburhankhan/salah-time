use super::{Database, STEPS, assert_preserved, check_integrity, original_rows, rows};
use crate::{Store, user_version};
use rusqlite::{Connection, ErrorCode, types::Value};

fn populated_v3(db: &Database) -> Connection {
    let mut conn = db.at(2, true);
    crate::migrations::run(&mut conn, None, &STEPS[..3]).unwrap();
    conn.execute_batch(
        "UPDATE items SET metrics_custom=1 WHERE id IN ('daily', 'once');
         UPDATE items SET rowid=19 WHERE id='daily';
         INSERT INTO item_metrics VALUES ('daily','pages',0), ('daily','focus',1);
         INSERT INTO session_notes VALUES
             ('n','work','2031-05-12T03:05:00Z','Page 12',
              '2031-05-12T03:05:00Z','2031-05-12T03:05:00Z',NULL,1);",
    )
    .unwrap();
    conn
}

#[test]
fn v4_preserves_every_v3_row_and_reference_in_place() {
    let db = Database::new("phase3");
    let seed = populated_v3(&db);
    let before = original_rows(&seed);
    let item_rowids = rows(&seed, "SELECT rowid,id FROM items ORDER BY rowid");
    drop(seed);

    let store = Store::open(&db.path).unwrap();
    let conn = &store.conn;
    assert_eq!(db.backups().len(), 1);
    let backup = db.backup(3);
    assert_preserved(&backup, &before);
    assert!(!backup.table_exists(None, "epics").unwrap());
    assert_eq!(user_version(conn).unwrap(), 4);
    assert!(store.foreign_keys_enabled().unwrap());
    assert_preserved(conn, &before);
    assert_eq!(
        rows(conn, "SELECT rowid,id FROM items ORDER BY rowid"),
        item_rowids
    );
    for table in ["items", "sessions", "day_plan"] {
        assert_eq!(
            rows(conn, &format!("SELECT DISTINCT epic_id FROM {table}")),
            [[Value::Null]],
            "{table}"
        );
    }
    for table in ["epics", "resources"] {
        assert!(rows(conn, &format!("SELECT * FROM {table}")).is_empty());
    }

    for (child, column, parent) in [
        ("sessions", "item_id", "items"),
        ("day_plan", "item_id", "items"),
        ("item_metrics", "item_id", "items"),
        ("item_history", "item_id", "items"),
        ("resources", "item_id", "items"),
        ("items", "project_id", "projects"),
        ("epics", "project_id", "projects"),
        ("items", "epic_id", "epics"),
        ("sessions", "epic_id", "epics"),
        ("day_plan", "epic_id", "epics"),
        ("resources", "epic_id", "epics"),
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
    for (index, columns) in [
        ("items_by_project", "project_id"),
        ("epics_by_project", "project_id,position"),
        ("sessions_by_epic_day", "epic_id,day"),
        ("resources_by_item", "item_id,position"),
        ("resources_by_epic", "epic_id,position"),
        ("resources_due", "kind,done,remind_at"),
    ] {
        assert_eq!(
            rows(
                conn,
                &format!(
                    "SELECT group_concat(name, ',' ORDER BY seqno) FROM pragma_index_info('{index}')"
                )
            ),
            [[Value::Text(columns.into())]],
            "{index}"
        );
    }
}

#[test]
fn v4_enforces_schedules_epic_status_and_resource_ownership() {
    let db = Database::new("phase3-constraints");
    drop(populated_v3(&db));
    let store = Store::open(&db.path).unwrap();
    let conn = &store.conn;
    conn.execute_batch(
        "INSERT INTO epics (id, project_id, name, status, position, created_at, updated_at, version)
         VALUES ('e','p','Read the series','active',0,'2031-05-14','2031-05-14',1);
         UPDATE epics SET status='finished', finished_at='2031-05-15' WHERE id='e';
         UPDATE items SET block=NULL, start_day=NULL, epic_id='e' WHERE id='once';
         UPDATE items SET epic_id='e' WHERE id='daily';
         UPDATE sessions SET epic_id='e' WHERE id='work';
         UPDATE day_plan SET epic_id='e' WHERE item_id='daily';
         INSERT INTO resources
             (id,item_id,epic_id,kind,body,url,remind_at,position,created_at,updated_at,version)
         VALUES ('n','once',NULL,'note','Library hours',NULL,NULL,0,'2031-05-14','2031-05-14',1),
                ('l',NULL,'e','link',NULL,'https://example.org',NULL,0,'2031-05-14','2031-05-14',1),
                ('r','daily',NULL,'reminder',NULL,NULL,'2031-05-15T14:00:00Z',0,'2031-05-14','2031-05-14',1);",
    )
    .unwrap();
    for sql in [
        "UPDATE items SET start_day='2031-05-15' WHERE id='once'",
        "UPDATE items SET block='isha' WHERE id='once'",
        "UPDATE items SET block=NULL,start_day=NULL WHERE id='daily'",
        "UPDATE items SET block=NULL,start_day=NULL WHERE id='weekly'",
        "UPDATE items SET block='sleep' WHERE id='daily'",
        "UPDATE items SET weekdays=128 WHERE id='weekly'",
        "UPDATE items SET weekdays=1 WHERE id='daily'",
        "UPDATE items SET project_id=NULL WHERE id='daily'",
        "UPDATE sessions SET project_id=NULL WHERE id='work'",
        "UPDATE epics SET project_id=NULL",
        "UPDATE epics SET status='active'",
        "UPDATE epics SET finished_at=NULL",
        "UPDATE epics SET status='paused',finished_at=NULL",
        "UPDATE epics SET version=0",
        "UPDATE resources SET epic_id='e' WHERE id='n'",
        "UPDATE resources SET item_id=NULL WHERE id='n'",
        "UPDATE resources SET kind='other' WHERE id='n'",
        "UPDATE resources SET body=NULL WHERE id='n'",
        "UPDATE resources SET url=NULL WHERE id='l'",
        "UPDATE resources SET remind_at=NULL WHERE id='r'",
        "UPDATE resources SET done=2 WHERE id='r'",
        "UPDATE resources SET version=0 WHERE id='r'",
    ] {
        let err = conn.execute(sql, []).expect_err(sql);
        assert_eq!(
            err.sqlite_error_code(),
            Some(ErrorCode::ConstraintViolation),
            "{sql}"
        );
    }
    check_integrity(conn);
}

#[test]
fn v4_rolls_back_the_rebuild_if_a_v3_reference_is_broken() {
    let db = Database::new("phase3-rollback");
    let mut conn = populated_v3(&db);
    conn.execute_batch(
        "PRAGMA foreign_keys=OFF;
         UPDATE item_history SET item_id='missing' WHERE item_id='daily';
         PRAGMA foreign_keys=ON;",
    )
    .unwrap();
    let before = original_rows(&conn);
    let error = crate::migrations::apply(&mut conn, Some(&db.path)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("violates a foreign key in item_history"),
        "{error}"
    );
    assert_eq!(user_version(&conn).unwrap(), 3);
    assert_eq!(rows(&conn, "PRAGMA foreign_keys"), [[Value::Integer(1)]]);
    assert!(db.backups().is_empty());
    assert!(!conn.table_exists(None, "epics").unwrap());
    assert!(!conn.column_exists(None, "items", "epic_id").unwrap());
    assert_eq!(original_rows(&conn), before);

    conn.execute(
        "UPDATE item_history SET item_id='daily' WHERE item_id='missing'",
        [],
    )
    .unwrap();
    let repaired = original_rows(&conn);
    let store = Store::open(&db.path).unwrap();
    assert_eq!(store.version().unwrap(), 4);
    assert_preserved(&store.conn, &repaired);
}
