use super::{
    DATE, Fixtures, Harness, LATER, NOW, Session,
    phase2::{added, bad, good},
};
use serde_json::{Value, json};

#[test]
fn finished_timed_notes_validate_span_and_support_versioned_idempotent_edits() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let task = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"task","title":"Read"}),
    );
    let id = added(
        &mut rpc,
        "session.start",
        "sessions",
        json!({"op_id":"start","item_id":task}),
    );
    let other = added(
        &mut rpc,
        "session.log",
        "sessions",
        json!({"op_id":"other","item_id":task,"minutes":10}),
    );
    drop(rpc);
    let mut rpc = Session::open(&h, LATER);
    let stopped = good(
        &mut rpc,
        "session.stop",
        json!({"op_id":"stop","note":"Main note"}),
    );
    let version = stopped["changed"]["sessions"][0]["version"]
        .as_i64()
        .unwrap();
    assert_eq!(version, 2);
    for at in ["9:40", "09:29", "09:56", "24:00"] {
        bad(
            &mut rpc,
            "session.note",
            json!({"op_id":"note","id":id,"text":"text","at":at}),
            "validation",
            Some("at"),
        );
    }
    bad(
        &mut rpc,
        "session.note",
        json!({"op_id":"note","id":id,"text":" ","at":"09:40"}),
        "validation",
        Some("text"),
    );
    let mut fixtures = Fixtures::with_ids(&[&id, &task]);
    let appended=fixtures.check("session_note_at",rpc.request("session.note",json!({"op_id":"note","id":id,"text":" First thought ","at":"09:40","expected_version":version})));
    let view = &appended["changed"]["sessions"][0];
    assert_eq!(view["version"], version + 1);
    assert_eq!(view["notes"][0]["at"], "2031-05-14T04:40:00Z");
    assert_eq!(view["notes"][0]["text"], "First thought");
    let note_id = view["notes"][0]["id"].as_str().unwrap();
    assert_eq!(
        good(
            &mut rpc,
            "session.note",
            json!({"op_id":"note","at":"nonsense"})
        ),
        appended
    );
    bad(
        &mut rpc,
        "session.note_edit",
        json!({"op_id":"edit","session_id":id,"note_id":note_id,"text":"Revised","expected_version":version}),
        "conflict",
        None,
    );
    bad(
        &mut rpc,
        "session.note_edit",
        json!({"op_id":"edit","session_id":id,"note_id":note_id,"text":" ","expected_version":version+1}),
        "validation",
        Some("text"),
    );
    bad(
        &mut rpc,
        "session.note_edit",
        json!({"op_id":"edit","session_id":other,"note_id":note_id,"text":"Revised","expected_version":1}),
        "not_found",
        None,
    );
    let edited=fixtures.check("session_note_edit",rpc.request("session.note_edit",json!({"op_id":"edit","session_id":id,"note_id":note_id,"text":"Revised","expected_version":version+1})));
    assert_eq!(
        edited["changed"]["sessions"][0]["notes"][0]["at"],
        view["notes"][0]["at"]
    );
    assert_eq!(
        edited["changed"]["sessions"][0]["notes"][0]["text"],
        "Revised"
    );
    assert_eq!(edited["changed"]["sessions"][0]["version"], version + 2);
    assert_eq!(
        good(&mut rpc, "session.note_edit", json!({"op_id":"edit"})),
        edited
    );
    bad(
        &mut rpc,
        "session.note_remove",
        json!({"op_id":"remove","session_id":id,"note_id":note_id,"expected_version":version+1}),
        "conflict",
        None,
    );
    let removed=fixtures.check("session_note_remove",rpc.request("session.note_remove",json!({"op_id":"remove","session_id":id,"note_id":note_id,"expected_version":version+2})));
    assert_eq!(removed["changed"]["sessions"][0]["notes"], json!([]));
    assert_eq!(removed["changed"]["sessions"][0]["note"], "Main note");
    assert_eq!(removed["changed"]["sessions"][0]["version"], version + 3);
    assert_eq!(
        good(&mut rpc, "session.note_remove", json!({"op_id":"remove"})),
        removed
    );
    bad(
        &mut rpc,
        "session.note_remove",
        json!({"op_id":"try-again","session_id":id,"note_id":note_id,"expected_version":version+3}),
        "not_found",
        None,
    );
    bad(
        &mut rpc,
        "session.note",
        json!({"op_id":"manual","id":other,"at":"23:59","text":"Outside Fajr"}),
        "validation",
        Some("at"),
    );
    let manual = good(
        &mut rpc,
        "session.note",
        json!({"op_id":"manual","id":other,"at":"09:35","text":"Manual log entry"}),
    );
    assert_eq!(
        manual["changed"]["sessions"][0]["notes"][0]["at"],
        "2031-05-14T04:35:00Z"
    );
    let store = salah_store::Store::open(&h.data).unwrap();
    assert_eq!(store.session(&id).unwrap().unwrap().version, version + 3);
    assert_eq!(
        good(&mut rpc, "session.notes", json!({"id":id}))["notes"],
        json!([])
    );
}

#[test]
fn note_time_after_midnight_uses_the_sessions_frozen_clock() {
    let h = Harness::new();
    let mut config = salah_core::config::load_from(&h.config).unwrap();
    config.sleep_time = "02:00".into();
    salah_core::config::save_to(&config, &h.config).unwrap();
    let mut rpc = Session::open(&h, "2031-05-14T23:50:20+05:00");
    let task = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"task","title":"Read"}),
    );
    let id = added(
        &mut rpc,
        "session.start",
        "sessions",
        json!({"op_id":"start","item_id":task}),
    );
    drop(rpc);
    let mut rpc = Session::open(&h, "2031-05-15T00:15:00+05:00");
    good(&mut rpc, "session.stop", json!({"op_id":"stop"}));
    drop(rpc);
    let mut rpc = Session::open(&h, "2031-05-15T09:30:00+05:00");
    good(&mut rpc, "day.get", json!({"date":DATE}));
    good(
        &mut rpc,
        "config.set",
        json!({"op_id":"tz","timezone_offset":3.0}),
    );
    let note = good(
        &mut rpc,
        "session.note",
        json!({"op_id":"after-midnight","id":id,"at":"00:05","text":"After midnight"}),
    );
    assert_eq!(
        note["changed"]["sessions"][0]["notes"][0]["at"],
        "2031-05-14T19:05:00Z"
    );
    let note = good(
        &mut rpc,
        "session.note",
        json!({"op_id":"start-minute","id":id,"at":"23:50","text":"First minute"}),
    );
    assert_eq!(
        note["changed"]["sessions"][0]["notes"][0]["at"],
        "2031-05-14T18:50:20Z"
    );
}

#[test]
fn note_rows_search_only_their_own_text_and_share_insights_measures_and_scope() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let tag = added(
        &mut rpc,
        "tag.add",
        "tags",
        json!({"op_id":"tag","name":"Study","color":"#123456"}),
    );
    let project = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"project","name":"Study","tag_ids":[tag]}),
    );
    let epic = added(
        &mut rpc,
        "epic.add",
        "epics",
        json!({"op_id":"epic","project_id":project,"name":"Book"}),
    );
    let item = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"item","epic_id":epic,"title":"Read"}),
    );
    let timer = added(
        &mut rpc,
        "session.start",
        "sessions",
        json!({"op_id":"start","item_id":item}),
    );
    drop(rpc);
    let mut rpc = Session::open(&h, LATER);
    good(
        &mut rpc,
        "session.stop",
        json!({"op_id":"stop","note":"Ärger in the main note","minutes":20,"values":{"quality":3,"focus":4,"distractions":2}}),
    );
    let entry = good(
        &mut rpc,
        "session.note",
        json!({"op_id":"add-entry","id":timer,"at":"09:40","text":"Ärger in a timed entry"}),
    );
    let note_id = entry["changed"]["sessions"][0]["notes"][0]["id"]
        .as_str()
        .unwrap();
    good(
        &mut rpc,
        "session.note",
        json!({"op_id":"different-entry","id":timer,"at":"09:45","text":"Different text"}),
    );
    for (op, minutes, values) in [
        (
            "slow",
            Some(40),
            json!({"focus":5,"quality":3,"distractions":2}),
        ),
        ("missing", Some(15), json!({"focus":5,"quality":3})),
        (
            "poor",
            Some(15),
            json!({"focus":5,"quality":2,"distractions":3}),
        ),
        (
            "tick",
            None,
            json!({"focus":5,"quality":3,"distractions":20}),
        ),
    ] {
        good(
            &mut rpc,
            "session.log",
            json!({"op_id":op,"item_id":item,"minutes":minutes,"note":"Ärger should not match","values":values}),
        );
    }
    let scope = json!({"from":DATE,"to":DATE,"project_id":project,"epic_id":epic,"tag_id":tag,"item_id":item});
    let unfiltered = good(&mut rpc, "insights.get", scope.clone());
    let mut filters = scope.clone();
    filters["minutes_min"] = json!(15);
    filters["minutes_max"] = json!(45);
    filters["values"] =
        json!({"quality":{"min":3},"focus":{},"distractions_per_hour":{"min":6,"max":6}});
    let mut fixtures = Fixtures::with_ids(&[&project, &epic, &item, &timer, note_id]);
    let report = fixtures.check(
        "insights_measures",
        rpc.request("insights.get", filters.clone()),
    );
    assert_eq!(report["by_hour"][9]["minutes"], 20);
    assert_eq!(report["by_weekday"][2]["minutes"], 20);
    assert_eq!(report["by_block"][1]["minutes"], 20);
    assert_eq!(report["distractions_by_week"][0]["per_hour"], 6.0);
    assert_eq!(report["best"][0]["id"], timer);
    assert_eq!(report["hardest"].as_array().unwrap().len(), 1);
    assert_eq!(report["on_plan"], unfiltered["on_plan"]);
    assert_eq!(report["block_matrix"], unfiltered["block_matrix"]);
    let sessions = good(&mut rpc, "sessions.list", filters.clone());
    assert_eq!(sessions["rows"].as_array().unwrap().len(), 1);
    assert_eq!(sessions["rows"][0]["id"], timer);
    assert_eq!(
        good(&mut rpc, "stats.table", filters.clone())["totals"]["minutes"],
        20
    );
    let present = good(
        &mut rpc,
        "sessions.list",
        json!({"days":1,"values":{"distractions_per_hour":{}}}),
    );
    assert_eq!(present["rows"].as_array().unwrap().len(), 3);
    good(
        &mut rpc,
        "project.edit",
        json!({"op_id":"archive","id":project,"archived":true}),
    );

    filters["q"] = json!(" ÄRGER ");
    filters["limit"] = json!(1);
    let first = fixtures.check("notes_list", rpc.request("notes.list", filters.clone()));
    assert_eq!(first["total"], 2);
    assert_eq!(first["rows"][0]["kind"], "main");
    assert!(first["rows"][0]["note_id"].is_null());
    assert_eq!(first["rows"][0]["session"]["project_id"], project);
    assert_eq!(first["rows"][0]["session"]["version"], 4);
    filters["before"] = first["next"].clone();
    let next = fixtures.check(
        "notes_list_next",
        rpc.request("notes.list", filters.clone()),
    );
    assert_eq!(next["total"], 2);
    assert_eq!(next["rows"][0]["note_id"], note_id);
    assert_eq!(next["rows"][0]["text"], "Ärger in a timed entry");
    assert_eq!(next["rows"][0]["at"], "2031-05-14T04:40:00Z");
    assert!(next["next"].is_null());

    filters["before"] = Value::Null;
    for (kind, text) in [
        ("main", "Ärger in the main note"),
        ("entry", "Ärger in a timed entry"),
    ] {
        filters["kind"] = json!(kind);
        let response = good(&mut rpc, "notes.list", filters.clone());
        assert_eq!(response["total"], 1);
        assert_eq!(response["rows"][0]["text"], text);
    }
    for (field, value) in [
        ("before", json!("invalid")),
        ("limit", json!(101)),
        ("values", json!({"distractions_per_hour":{"min":7,"max":6}})),
    ] {
        let mut params = filters.clone();
        params[field] = value;
        bad(&mut rpc, "notes.list", params, "validation", Some(field));
    }
    let none = good(
        &mut rpc,
        "notes.list",
        json!({"days":1,"project_id":"none","tag_id":"none"}),
    );
    assert_eq!(none, json!({"rows":[],"total":0,"next":null}));
}

#[test]
fn notes_pages_stay_below_the_frame_limit_and_skip_removed_or_running_work() {
    use salah_domain::tracking::{SessionNote, SessionValue};
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let item = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"item","title":"Read"}),
    );
    let finished = added(
        &mut rpc,
        "session.log",
        "sessions",
        json!({"op_id":"work","item_id":item,"minutes":20,"note":"Main"}),
    );
    let removed = added(
        &mut rpc,
        "session.log",
        "sessions",
        json!({"op_id":"removed","item_id":item,"minutes":10,"note":"Removed session"}),
    );
    good(
        &mut rpc,
        "session.remove",
        json!({"op_id":"remove","id":removed}),
    );
    good(
        &mut rpc,
        "session.start",
        json!({"op_id":"running","item_id":item}),
    );
    good(
        &mut rpc,
        "session.note",
        json!({"op_id":"running-note","text":"Running entry"}),
    );
    let mut store = salah_store::Store::open(&h.data).unwrap();
    let mut session = store.session(&finished).unwrap().unwrap();
    let mut metric = store.metric("distractions").unwrap().unwrap();
    metric.builtin = false;
    let at = NOW.parse().unwrap();
    let note_count = 100;
    store
        .write(|tx| {
            for index in 0..210 {
                metric.id = format!("00000000-0000-4000-8000-{index:012}");
                metric.name = format!("Count {index}");
                tx.put_metric(&metric)?;
                session.values.push(SessionValue {
                    metric_id: metric.id.clone(),
                    value: 3.0,
                });
            }
            tx.put_session(&session)?;
            for index in 0..=note_count {
                tx.add_session_note(
                    &finished,
                    &SessionNote {
                        id: format!("note-{index:03}"),
                        at,
                        text: "𐐀".repeat(500),
                    },
                )?;
            }
            tx.remove_session_notes(&finished, &["note-000".to_owned()], at)
        })
        .unwrap();
    let mut before = Value::Null;
    let mut seen = std::collections::BTreeSet::new();
    for page in 0..3 {
        let frame = rpc.request("notes.list", json!({"days":1,"limit":100,"before":before}));
        assert!(serde_json::to_vec(&frame).unwrap().len() + 1 < salah_protocol::MAX_LINE_BYTES);
        assert_eq!(frame["result"]["total"], note_count + 1);
        let rows = frame["result"]["rows"].as_array().unwrap();
        assert!(!rows.is_empty());
        if page == 0 {
            assert!(rows.len() < 100, "byte budget must shorten the first page");
        }
        for row in rows {
            assert_eq!(row["session_id"], finished);
            assert_ne!(row["note_id"], "note-000");
            assert!(seen.insert(row["note_id"].as_str().unwrap_or("main").to_owned()));
        }
        before = frame["result"]["next"].clone();
        if before.is_null() {
            break;
        }
    }
    assert!(before.is_null());
    assert_eq!(seen.len(), note_count + 1);
}

#[test]
fn timed_notes_keep_newlines_and_refuse_other_control_characters() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let task = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"task","title":"Read"}),
    );
    let id = added(
        &mut rpc,
        "session.start",
        "sessions",
        json!({"op_id":"start","item_id":task}),
    );
    let noted = good(
        &mut rpc,
        "session.note",
        json!({"op_id":"n1","id":id,"text":"line one\nline two"}),
    );
    let view = &noted["changed"]["sessions"][0];
    assert_eq!(view["notes"][0]["text"], "line one\nline two");
    let note_id = view["notes"][0]["id"].as_str().unwrap().to_owned();
    let version = view["version"].as_i64().unwrap();
    let edited = good(
        &mut rpc,
        "session.note_edit",
        json!({"op_id":"e1","session_id":id,"note_id":note_id,"text":"a\nb\nc","expected_version":version}),
    );
    assert_eq!(
        edited["changed"]["sessions"][0]["notes"][0]["text"],
        "a\nb\nc"
    );
    bad(
        &mut rpc,
        "session.note",
        json!({"op_id":"n2","id":id,"text":"tab\there"}),
        "validation",
        Some("text"),
    );
}
