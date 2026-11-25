use super::{Database, STEPS, assert_preserved, original_rows, phase4::populated_v4, rows};
use crate::{migrations, user_version};
use rusqlite::{ErrorCode, types::Value};

#[test]
fn v6_preserves_v5_data_and_history_and_checks_routine_end_dates() {
    let db = Database::new("routine-end");
    let mut conn = populated_v4(&db);
    migrations::run(&mut conn, None, &STEPS[..5]).unwrap();
    conn.execute_batch(
        "INSERT INTO goals(id,item_id,period,target,starts_on,position,created_at,updated_at,version)
         VALUES('read-goal','daily','week',60,'2031-05-12',0,'2031-05-12','2031-05-12',1);
         INSERT INTO reviews(id,week,note,created_at,updated_at,version)
         VALUES('review','2031-05-12','Finished the chapter','2031-05-18','2031-05-18',1);",
    ).unwrap();
    let before = original_rows(&conn);
    let item_rowids = rows(&conn, "SELECT rowid,id FROM items ORDER BY rowid");
    migrations::run(&mut conn, Some(&db.path), &STEPS[..6]).unwrap();
    assert_eq!(user_version(&conn).unwrap(), 6);
    let conn = &conn;
    assert_eq!(db.backups().len(), 1);
    let backup = db.backup(5);
    assert!(!backup.column_exists(None, "items", "end_day").unwrap());
    assert_preserved(&backup, &before);
    assert_preserved(conn, &before);
    assert_eq!(
        rows(conn, "SELECT rowid,id FROM items ORDER BY rowid"),
        item_rowids
    );
    assert_eq!(
        rows(conn, "SELECT DISTINCT end_day FROM items"),
        [[Value::Null]]
    );
    for (id, end) in [
        ("daily", "2031-05-09"),
        ("daily", "2032-02-30"),
        ("daily", "2031-99-99"),
        ("daily", "invalid"),
        ("backlog", "2031-05-12"),
        ("once", "2031-05-12"),
    ] {
        let error = conn
            .execute("UPDATE items SET end_day=?2 WHERE id=?1", [id, end])
            .unwrap_err();
        assert_eq!(
            error.sqlite_error_code(),
            Some(ErrorCode::ConstraintViolation)
        );
    }
    conn.execute_batch(
        "UPDATE items SET end_day=start_day WHERE id='daily';
                        UPDATE items SET end_day='2031-05-16' WHERE id='weekly';
                        UPDATE items SET end_day=NULL WHERE id='daily';",
    )
    .unwrap();
    let after = original_rows(conn);
    let backups = db.backups();
    let mut reopened = rusqlite::Connection::open(&db.path).unwrap();
    migrations::run(&mut reopened, Some(&db.path), &STEPS[..6]).unwrap();
    assert_preserved(&reopened, &after);
    assert_eq!(db.backups(), backups);
}

#[test]
fn a_v5_reader_refuses_v6_and_the_current_runner_refuses_newer() {
    let db = Database::new("routine-end-newer");
    let mut conn = populated_v4(&db);
    migrations::run(&mut conn, None, &STEPS[..6]).unwrap();
    let backups = db.backups();
    for (version, steps) in [
        (6, &STEPS[..5]),
        (7, &STEPS[..6]),
        (crate::schema_version() + 1, STEPS),
    ] {
        conn.pragma_update(None, "user_version", version).unwrap();
        let err = migrations::run(&mut conn, Some(&db.path), steps).unwrap_err();
        assert!(err.to_string().contains("unsupported schema version"));
        assert_eq!(user_version(&conn).unwrap(), version);
        assert_eq!(db.backups(), backups);
    }
}
