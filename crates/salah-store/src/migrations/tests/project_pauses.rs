use super::{Database, STEPS, assert_preserved, original_rows, phase4::populated_v4, rows};
use crate::{Store, migrations, user_version};
use rusqlite::types::Value;

#[test]
fn v7_preserves_all_v6_rows_and_backs_up_before_dating_archived_projects() {
    let db = Database::new("project-pauses");
    let mut conn = populated_v4(&db);
    migrations::run(&mut conn, None, &STEPS[..6]).unwrap();
    conn.execute_batch(
        "UPDATE items SET end_day='2031-06-01' WHERE id='daily';
        INSERT INTO goals(id,item_id,period,target,starts_on,position,created_at,updated_at,version)
        VALUES('g','daily','week',100,'2031-05-12',0,'2031-05-12','2031-05-12',1);
        INSERT INTO reviews(id,week,note,created_at,updated_at,version)
        VALUES('w','2031-05-12','Made progress','2031-05-18','2031-05-18',1);",
    )
    .unwrap();
    let before = original_rows(&conn);
    drop(conn);
    // Failure to resolve the tracking day leaves the v6 file and its backups intact.
    assert!(
        Store::open_with_day(&db.path, || Err(salah_domain::error::Error::Config(
            "clock unavailable".into()
        )))
        .is_err()
    );
    assert!(db.backups().is_empty());
    let conn = rusqlite::Connection::open(&db.path).unwrap();
    assert_eq!(user_version(&conn).unwrap(), 6);
    assert_preserved(&conn, &before);
    let store = Store::open_with_day(&db.path, || Ok("2031-05-15".parse().unwrap())).unwrap();
    assert_eq!(store.version().unwrap(), 7);
    assert_eq!(db.backups().len(), 1);
    let backup = db.backup(6);
    assert!(!backup.table_exists(None, "project_pauses").unwrap());
    assert_preserved(&backup, &before);
    assert_preserved(&store.conn, &before);
    assert_eq!(
        rows(
            &store.conn,
            "SELECT project_id,from_day,to_day FROM project_pauses"
        ),
        [vec![
            Value::Text("a".into()),
            Value::Text("2031-05-15".into()),
            Value::Null
        ]]
    );
    assert!(
        store
            .conn
            .execute(
                "INSERT INTO project_pauses VALUES('a','2031-05-16',NULL)",
                []
            )
            .is_err()
    );
    assert!(
        store
            .conn
            .execute("UPDATE project_pauses SET to_day='2031-05-14'", [])
            .is_err()
    );
    let backups = db.backups();
    drop(store);
    let _store = Store::open_with_day(&db.path, || panic!("already migrated")).unwrap();
    assert_eq!(db.backups(), backups);
}
