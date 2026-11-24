use super::{
    DATE, Fixtures, Harness, NOW, Session,
    phase2::{added, bad, good},
};
use serde_json::json;

#[test]
fn activity_groups_finished_live_routine_sessions_by_tracking_day() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let routine = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"routine","title":"Read","repeat":"daily"}),
    );
    let once = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"once","title":"Once"}),
    );
    let removed = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"removed","title":"Removed","repeat":"daily"}),
    );
    let project = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"p","name":"Study"}),
    );
    for (op, item, day, minutes) in [
        ("first", &routine, DATE, Some(10)),
        ("second", &routine, DATE, Some(20)),
        ("done", &routine, DATE, None),
        ("tick-only", &routine, "2031-05-13", None),
        ("duration", &routine, "2031-05-12", Some(7)),
        ("outside", &routine, "2031-05-11", Some(9)),
        ("one-time", &once, DATE, Some(12)),
        ("removed-log", &removed, DATE, Some(14)),
    ] {
        good(
            &mut rpc,
            "session.log",
            json!({"op_id":op,"item_id":item,"date":day,"minutes":minutes}),
        );
    }
    let deleted_tick = added(
        &mut rpc,
        "session.log",
        "sessions",
        json!({"op_id":"deleted-tick","item_id":routine,"date":"2031-05-12"}),
    );
    good(
        &mut rpc,
        "session.remove",
        json!({"op_id":"delete-tick","id":deleted_tick}),
    );
    good(
        &mut rpc,
        "item.remove",
        json!({"op_id":"delete-item","id":removed}),
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"bare","minutes":50,"project_id":project}),
    );
    let idle = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"idle","title":"Running only","repeat":"daily"}),
    );
    good(
        &mut rpc,
        "session.start",
        json!({"op_id":"timer","item_id":idle}),
    );
    let range = json!({"from":"2031-05-12","to":DATE});
    let result = Fixtures::default().check(
        "routines_activity",
        rpc.request("routines.activity", range.clone()),
    );
    assert_eq!(
        result,
        json!({"rows":[{"day":DATE,"item_id":routine,"minutes":30,"done":true},{"day":"2031-05-13","item_id":routine,"minutes":0,"done":true},{"day":"2031-05-12","item_id":routine,"minutes":7,"done":false}],"next":null})
    );
}

#[test]
fn activity_keyset_pages_across_items_and_days_and_checks_bounds() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let id = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"w","title":"Study","repeat":"weekly","weekdays":["wed"]}),
    );
    let sid = added(
        &mut rpc,
        "session.log",
        "sessions",
        json!({"op_id":"duration","item_id":id,"minutes":5}),
    );
    let mut store = salah_store::Store::open(&h.data).unwrap();
    let mut item = store.item(&id).unwrap().unwrap();
    let mut session = store.session(&sid).unwrap().unwrap();
    store
        .write(|tx| {
            for i in 0..501 {
                item.id = format!("routine-{i:03}");
                tx.put_item(&item)?;
                session.item_id = Some(item.id.clone());
                session.id = format!("work-{i:03}");
                tx.put_session(&session)?;
            }
            session.id = "yesterday".into();
            session.block.day = session.block.day.pred_opt().unwrap();
            tx.put_session(&session)?;
            Ok(())
        })
        .unwrap();
    good(
        &mut rpc,
        "item.remove",
        json!({"op_id":"remove-template","id":id}),
    );
    let from = "2031-05-01";
    let page = good(
        &mut rpc,
        "routines.activity",
        json!({"from":from,"to":DATE}),
    );
    let rows = page["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 500);
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row["item_id"], format!("routine-{i:03}"));
        assert_eq!(row["day"], DATE);
    }
    let cursor = page["next"].as_str().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_salah"))
        .env("SALAH_CONFIG", &h.config)
        .env("SALAH_DATA", &h.data)
        .env("SALAH_NOW", NOW)
        .args([
            "routines", "activity", "--from", from, "--to", DATE, "--limit", "500", "--before",
            cursor, "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let next: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        next,
        json!({"rows":[{"day":DATE,"item_id":"routine-500","minutes":5,"done":false},{"day":"2031-05-13","item_id":"routine-500","minutes":5,"done":false}],"next":null})
    );
    for (field, value) in [
        ("from", json!("2031-05-1")),
        ("to", json!("2031-04-30")),
        ("to", json!("2031-05-15")),
        ("before", json!("bad cursor")),
        ("limit", json!(0)),
        ("limit", json!(501)),
    ] {
        let mut params = json!({"from":from,"to":DATE});
        params[field] = value;
        bad(
            &mut rpc,
            "routines.activity",
            params,
            "validation",
            Some(field),
        );
    }
}
