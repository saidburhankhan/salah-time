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

/// Extract the matching task from all the blocks in the requested day view.
fn planned(rpc: &mut Session, day: &str, id: &str) -> Option<serde_json::Value> {
    good(rpc, "day.get", json!({"date":day}))["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|block| block["items"].as_array().unwrap())
        .find(|item| item["id"] == id)
        .cloned()
}

#[test]
fn converting_to_once_keeps_written_and_unwritten_occurrences_with_sessions() {
    let h = Harness::new();
    let days = ["2029-05-11", "2029-05-12", "2029-05-13"];
    let mut rpc = Session::open(&h, "2029-05-11T09:30:00+05:00");
    let id = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"create","title":"Read","repeat":"daily","block":"fajr"}),
    );
    drop(rpc);
    let mut logs = Vec::new();
    for (i, day) in days.iter().enumerate() {
        let mut rpc = Session::open(&h, &format!("{day}T09:30:00+05:00"));
        logs.push(added(
            &mut rpc,
            "session.log",
            "sessions",
            json!({"op_id":format!("log-{i}"),"item_id":id,"minutes":i+1}),
        ));
        if i == 1 {
            good(
                &mut rpc,
                "session.log",
                json!({"op_id":"completion","item_id":id}),
            );
            assert!(planned(&mut rpc, days[0], &id).is_some());
        }
    }
    // The first day was materialized. The other two are outside freeze_past's year
    // and can only be reconstructed from item_history after the conversion.
    let mut rpc = Session::open(&h, NOW);
    let result = good(
        &mut rpc,
        "item.edit",
        json!({"op_id":"convert","id":id,"repeat":"none","title":"Finish the book","block":"asr","expected_version":1}),
    );
    let once = &result["changed"]["items"][0];
    assert_eq!(once["day"], DATE);
    assert_eq!(once["next_occurrence"], DATE);
    let store = salah_store::Store::open(&h.data).unwrap();
    for (i, day) in days.iter().enumerate() {
        assert_eq!(store.day_written(day.parse().unwrap()).unwrap(), i == 0);
        let row = planned(&mut rpc, day, &id).unwrap();
        assert_eq!(row["repeat"], "daily");
        assert_eq!(row["title"], "Read");
        assert_eq!(row["block"], "fajr");
        assert_eq!(row["actual_minutes"], i + 1);
        assert_eq!(row["done"], i == 1);
        assert!(
            row["sessions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["id"] == logs[i])
        );
    }
    let today = planned(&mut rpc, DATE, &id).unwrap();
    assert_eq!(today["title"], "Finish the book");
    assert_eq!(today["repeat"], "none");
    assert_eq!(today["block"], "asr");
    assert!(today["sessions"].as_array().unwrap().is_empty());
    assert!(planned(&mut rpc, "2031-05-15", &id).is_none());
}

#[test]
fn end_day_is_inclusive_resume_keeps_the_gap_and_conversion_accepts_a_chosen_date() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let id = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"start","title":"Read","repeat":"weekly","weekdays":["wed","thu","fri","sat"]}),
    );
    for end in ["2031-05-13", "2031-02-29", "2031-5-15"] {
        bad(
            &mut rpc,
            "item.edit",
            json!({"op_id":"invalid","id":id,"end_day":end}),
            "validation",
            Some("end_day"),
        );
    }
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"last-log","item_id":id,"minutes":12}),
    );
    let res = good(
        &mut rpc,
        "item.edit",
        json!({"op_id":"end","id":id,"end_day":DATE,"expected_version":1}),
    );
    let row = &res["changed"]["items"][0];
    assert_eq!(row["end_day"], DATE);
    assert_eq!(row["ended"], false);
    assert_eq!(row["next_occurrence"], DATE);
    assert!(planned(&mut rpc, "2031-05-15", &id).is_none());
    drop(rpc);

    // After Sleep on Wednesday, Thursday is the current tracking day.
    let mut thursday = Session::open(&h, "2031-05-15T00:45:00+05:00");
    assert!(
        good(&mut thursday, "items.list", json!({}))
            .as_array()
            .unwrap()
            .is_empty()
    );
    let listed = good(&mut thursday, "items.list", json!({"include_ended":true}));
    assert_eq!(listed[0]["ended"], true);
    let activity = good(
        &mut thursday,
        "routines.activity",
        json!({"from":DATE,"to":"2031-05-15"}),
    );
    assert_eq!(
        activity["rows"],
        json!([{"item_id":id,"day":DATE,"minutes":12,"done":false}])
    );
    assert!(listed[0]["next_occurrence"].is_null());
    assert_eq!(
        planned(&mut thursday, DATE, &id).unwrap()["actual_minutes"],
        12
    );
    assert!(planned(&mut thursday, "2031-05-15", &id).is_none());
    drop(thursday);

    let mut friday = Session::open(&h, "2031-05-16T09:30:00+05:00");
    let res = good(
        &mut friday,
        "item.edit",
        json!({"op_id":"resume","id":id,"end_day":null,"expected_version":2}),
    );
    let resumed = &res["changed"]["items"][0];
    assert!(resumed["end_day"].is_null());
    assert_eq!(resumed["ended"], false);
    assert_eq!(resumed["next_occurrence"], "2031-05-16");
    assert!(planned(&mut friday, "2031-05-15", &id).is_none());
    assert!(planned(&mut friday, "2031-05-16", &id).is_some());

    bad(
        &mut friday,
        "item.edit",
        json!({"op_id":"past","id":id,"repeat":"none","date":DATE}),
        "validation",
        Some("date"),
    );
    let res = good(
        &mut friday,
        "item.edit",
        json!({"op_id":"once","id":id,"repeat":"none","date":"2031-05-18","expected_version":3}),
    );
    assert_eq!(res["changed"]["items"][0]["day"], "2031-05-18");
    assert!(planned(&mut friday, "2031-05-16", &id).is_none());
    assert!(planned(&mut friday, "2031-05-17", &id).is_none());
    assert_eq!(
        planned(&mut friday, "2031-05-18", &id).unwrap()["repeat"],
        "none"
    );
    assert_eq!(planned(&mut friday, DATE, &id).unwrap()["repeat"], "weekly");
    bad(
        &mut friday,
        "item.edit",
        json!({"op_id":"end-once","id":id,"end_day":"2031-05-20"}),
        "validation",
        Some("end_day"),
    );
}
