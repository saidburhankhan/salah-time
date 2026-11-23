use super::{DATE, Fixtures, Harness, LATER, NOW, Session, id};
use serde_json::{Value, json};

pub(super) fn good(rpc: &mut Session, method: &str, params: Value) -> Value {
    let response = rpc.request(method, params);
    assert!(response.get("error").is_none(), "{method}: {response}");
    response["result"].clone()
}

pub(super) fn bad(rpc: &mut Session, method: &str, params: Value, kind: &str, field: Option<&str>) {
    let response = rpc.request(method, params);
    assert_eq!(response["error"]["data"]["kind"], kind, "{response}");
    if let Some(field) = field {
        assert_eq!(response["error"]["data"]["field"], field, "{response}");
    }
}

pub(super) fn added(rpc: &mut Session, method: &str, kind: &str, params: Value) -> String {
    id(&good(rpc, method, params), kind)
}

fn metric_ids(metrics: &Value) -> Vec<&str> {
    metrics
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["metric_id"].as_str().unwrap())
        .collect()
}

#[test]
fn stats_item_filter_intersects_project_and_tag_and_supports_730_inclusive_days() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let tag = added(
        &mut rpc,
        "tag.add",
        "tags",
        json!({"op_id":"tag","name":"Study","color":"#223344"}),
    );
    let other_tag = added(
        &mut rpc,
        "tag.add",
        "tags",
        json!({"op_id":"other-tag","name":"Other","color":"#334455"}),
    );
    let project = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"project","name":"Quran","tag_ids":[tag]}),
    );
    let item = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"item","title":"Read","project_id":project,"planned_minutes":20}),
    );
    let sibling = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"sibling","title":"Review","project_id":project,"planned_minutes":40}),
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"log","item_id":item,"minutes":15,"values":{"focus":4}}),
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"other-log","item_id":sibling,"minutes":25,"values":{"focus":2}}),
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"project-log","project_id":project,"minutes":20}),
    );
    good(
        &mut rpc,
        "session.start",
        json!({"op_id":"start","item_id":item}),
    );
    let response = rpc.request(
        "stats.get",
        json!({"from":DATE,"to":DATE,"tag_id":tag,"project_id":project,"item_id":item}),
    );
    let filter = Fixtures::with_ids(&[&item, &project, &tag]).check("stats_item_filter", response);
    assert_eq!(filter["filter"]["item_id"], item);
    assert_eq!(filter["totals"]["minutes"], 15);
    assert_eq!(filter["totals"]["sessions"], 1);
    assert_eq!(filter["totals"]["avg_focus"], 4.0);
    assert_eq!(filter["days"][0]["planned_minutes"], 20);
    assert_eq!(filter["days"][0]["fulfilment_ratio"], 0.75);
    assert_eq!(filter["projects"][0]["minutes"], 15);
    let tags = filter["tags"].as_array().unwrap();
    assert_eq!(
        tags.iter().find(|row| row["id"] == tag).unwrap()["minutes"],
        15
    );
    assert_eq!(
        tags.iter().find(|row| row["id"] == other_tag).unwrap()["minutes"],
        0
    );
    let excluded = good(
        &mut rpc,
        "stats.get",
        json!({"days":1,"item_id":item,"tag_id":other_tag}),
    );
    assert_eq!(excluded["totals"]["sessions"], 0);
    bad(
        &mut rpc,
        "stats.get",
        json!({"days":1,"item_id":"missing"}),
        "not_found",
        None,
    );

    let from = chrono::NaiveDate::parse_from_str(DATE, "%Y-%m-%d").unwrap();
    let to = |days| (from + chrono::Days::new(days)).to_string();
    let max = good(
        &mut rpc,
        "stats.get",
        json!({"from":DATE,"to":to(729),"item_id":item}),
    );
    assert_eq!(max["days"].as_array().unwrap().len(), 730);
    bad(
        &mut rpc,
        "stats.get",
        json!({"from":DATE,"to":to(730)}),
        "validation",
        Some("from"),
    );
    bad(
        &mut rpc,
        "stats.get",
        json!({"days":731}),
        "validation",
        Some("days"),
    );
}

#[test]
fn stop_overrides_minutes_and_save_done_returns_a_replayable_empty_tick() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let item = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"item","title":"Read"}),
    );
    let timer = added(
        &mut rpc,
        "session.start",
        "sessions",
        json!({"op_id":"start","item_id":item}),
    );
    drop(rpc);
    let mut rpc = Session::open(&h, LATER);
    for minutes in [0, 26] {
        bad(
            &mut rpc,
            "session.stop",
            json!({"op_id":"shorten","minutes":minutes}),
            "validation",
            Some("minutes"),
        );
        assert_eq!(
            good(&mut rpc, "status.get", json!({}))["running"]["id"],
            timer
        );
    }
    let mut fixtures = Fixtures::default();
    let minutes = fixtures.check(
        "session_stop_minutes",
        rpc.request("session.stop", json!({"op_id":"shorten","minutes":10})),
    );
    let stopped = &minutes["changed"]["sessions"][0];
    assert_eq!(stopped["minutes"], 10);
    assert_eq!(stopped["ended_at"], "2031-05-14T04:55:00Z");

    good(
        &mut rpc,
        "session.start",
        json!({"op_id":"again","item_id":item}),
    );
    let params = json!({"op_id":"save-done","minutes":1,"done":true,"note":"Finished a chapter","values":{"focus":5}});
    let result = fixtures.check("session_stop_done", rpc.request("session.stop", params));
    assert_eq!(
        good(
            &mut rpc,
            "session.stop",
            json!({"op_id":"save-done","done":"invalid replay body"})
        ),
        result
    );
    let sessions = result["changed"]["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0]["minutes"], 1);
    assert_eq!(sessions[0]["note"], "Finished a chapter");
    assert_eq!(sessions[0]["values"], json!({"focus":5.0}));
    let tick = &sessions[1];
    assert!(tick["minutes"].is_null() && tick["note"].is_null());
    assert_eq!(tick["values"], json!({}));
    let store = salah_store::Store::open(&h.data).unwrap();
    let changes = store.changes_since(0, 100).unwrap();
    let relevant: Vec<_> = changes
        .iter()
        .filter(|c| c.op_id.as_deref() == Some("save-done"))
        .collect();
    assert_eq!(relevant.len(), 2);
    for change in relevant {
        assert_eq!(change.revision, result["revision"]);
    }

    good(
        &mut rpc,
        "session.start",
        json!({"op_id":"after-done","item_id":item}),
    );
    let another = good(
        &mut rpc,
        "session.stop",
        json!({"op_id":"still-done","done":true}),
    );
    assert_eq!(&another["changed"]["sessions"][1], tick);

    let project = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"project","name":"Book"}),
    );
    let timer = added(
        &mut rpc,
        "session.start",
        "sessions",
        json!({"op_id":"bare-start","project_id":project}),
    );
    bad(
        &mut rpc,
        "session.stop",
        json!({"op_id":"bare-done","done":true}),
        "validation",
        Some("done"),
    );
    assert_eq!(
        good(&mut rpc, "status.get", json!({}))["running"]["id"],
        timer
    );
}

#[test]
fn effective_item_metrics_follow_project_order_and_cleanup_persists() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let pages = added(
        &mut rpc,
        "metric.add",
        "metrics",
        json!({"op_id":"pages","name":"Pages","kind":"count"}),
    );
    let other = added(
        &mut rpc,
        "metric.add",
        "metrics",
        json!({"op_id":"other","name":"Other","kind":"count"}),
    );
    let project = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"project","name":"Book",
        "metrics":[{"metric_id":pages,"required":false},{"metric_id":"focus","required":true}]}),
    );
    let item = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"item","title":"Read","project_id":project,"metric_ids":[]}),
    );
    let item_list = |rpc: &mut Session| good(rpc, "items.list", json!({}));
    assert_eq!(item_list(&mut rpc)[0]["metric_ids"], json!([]));
    assert_eq!(item_list(&mut rpc)[0]["metrics"], json!([]));

    for rejected in [json!([other]), json!(["focus", "focus"])] {
        bad(
            &mut rpc,
            "item.edit",
            json!({"op_id":"edit","id":item,"metric_ids":rejected}),
            "validation",
            Some("metric_ids"),
        );
    }
    let selected = json!(["quality", "focus", pages]);
    let result = Fixtures::default().check(
        "item_edit_metrics",
        rpc.request(
            "item.edit",
            json!({"op_id":"edit","id":item,"metric_ids":selected}),
        ),
    );
    let row = &result["changed"]["items"][0];
    assert_eq!(row["metric_ids"], selected);
    assert_eq!(
        metric_ids(&row["metrics"]),
        [pages.as_str(), "focus", "quality"]
    );
    assert!(
        row["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["required"] == false)
    );

    good(
        &mut rpc,
        "session.start",
        json!({"op_id":"start","item_id":item}),
    );
    let status = good(&mut rpc, "status.get", json!({}));
    assert_eq!(status["prompt"], row["metrics"]);
    assert_eq!(status["planner"]["items"][0]["metric_ids"], selected);
    assert_eq!(status["planner"]["items"][0]["metrics"], row["metrics"]);
    good(
        &mut rpc,
        "metric.edit",
        json!({"op_id":"archive","id":pages,"archived":true}),
    );
    assert_eq!(
        metric_ids(&good(&mut rpc, "status.get", json!({}))["prompt"]),
        ["focus", "quality"]
    );
    bad(
        &mut rpc,
        "item.edit",
        json!({"op_id":"choose-archived","id":item,"metric_ids":[pages]}),
        "validation",
        Some("metric_ids"),
    );

    let cleanup = good(
        &mut rpc,
        "project.edit",
        json!({"op_id":"drop-pages","id":project,"metrics":[{"metric_id":"focus","required":true}]}),
    );
    assert_eq!(
        cleanup["changed"]["items"][0]["metric_ids"],
        json!(["quality", "focus"])
    );
    assert_eq!(cleanup["changed"]["items"][0]["version"], 3);
    drop(rpc);
    let mut rpc = Session::open(&h, NOW);
    assert_eq!(
        item_list(&mut rpc)[0]["metric_ids"],
        json!(["quality", "focus"])
    );
    let reset = good(
        &mut rpc,
        "item.edit",
        json!({"op_id":"inherit","id":item,"metric_ids":null}),
    );
    assert!(reset["changed"]["items"][0]["metric_ids"].is_null());
    assert_eq!(
        metric_ids(&reset["changed"]["items"][0]["metrics"]),
        ["focus"]
    );

    good(
        &mut rpc,
        "item.edit",
        json!({"op_id":"select","id":item,"metric_ids":["quality"]}),
    );
    let switched = good(
        &mut rpc,
        "item.edit",
        json!({"op_id":"leave-project","id":item,"project_id":null,"metric_ids":["quality"]}),
    );
    // metric_ids sent with a project change apply to the new project (R1).
    assert_eq!(
        switched["changed"]["items"][0]["metric_ids"],
        json!(["quality"])
    );
    assert_eq!(
        metric_ids(&switched["changed"]["items"][0]["metrics"]),
        ["quality"]
    );
    assert_eq!(
        good(&mut rpc, "status.get", json!({}))["running"]["project_id"],
        project
    );
    assert_eq!(
        metric_ids(&good(&mut rpc, "status.get", json!({}))["prompt"]),
        ["quality"]
    );
    let reset = good(
        &mut rpc,
        "item.edit",
        json!({"op_id":"rejoin-project","id":item,"project_id":project}),
    );
    assert!(reset["changed"]["items"][0]["metric_ids"].is_null());
}

#[test]
fn bumps_and_notes_update_a_running_session_and_replay_without_duplicates() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let item = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"item","title":"Read","metric_ids":[]}),
    );
    let timer = added(
        &mut rpc,
        "session.start",
        "sessions",
        json!({"op_id":"start","item_id":item}),
    );
    let mut fixtures = Fixtures::default();
    bad(
        &mut rpc,
        "session.bump",
        json!({"op_id":"bump","metric_id":"focus","delta":1}),
        "validation",
        Some("metric_id"),
    );
    bad(
        &mut rpc,
        "session.bump",
        json!({"op_id":"bump","metric_id":"distractions","delta":0}),
        "validation",
        Some("delta"),
    );
    let bump = fixtures.check(
        "session_bump",
        rpc.request(
            "session.bump",
            json!({"op_id":"bump","metric_id":"distractions","delta":1}),
        ),
    );
    assert_eq!(
        bump["changed"]["sessions"][0]["values"]["distractions"],
        1.0
    );
    assert_eq!(
        good(&mut rpc, "session.bump", json!({"op_id":"bump","delta":-1})),
        bump
    );
    for op in ["down", "floor"] {
        let down = good(
            &mut rpc,
            "session.bump",
            json!({"op_id":op,"id":timer,"metric_id":"distractions","delta":-1}),
        );
        assert_eq!(
            down["changed"]["sessions"][0]["values"]["distractions"],
            0.0
        );
    }
    good(
        &mut rpc,
        "session.bump",
        json!({"op_id":"keep","metric_id":"distractions","delta":1}),
    );
    good(
        &mut rpc,
        "session.edit",
        json!({"op_id":"main","id":timer,"note":"Summary"}),
    );
    for text in ["  ".to_owned(), "x".repeat(501)] {
        bad(
            &mut rpc,
            "session.note",
            json!({"op_id":"note","text":text}),
            "validation",
            Some("text"),
        );
    }
    let first = good(
        &mut rpc,
        "session.note",
        json!({"op_id":"note","text":"  First thought  "}),
    );
    let first_id = first["changed"]["sessions"][0]["notes"][0]["id"].clone();
    assert_eq!(
        first["changed"]["sessions"][0]["notes"][0]["text"],
        "First thought"
    );
    assert_eq!(first["changed"]["sessions"][0]["note"], "Summary");
    drop(rpc);

    let mut rpc = Session::open(&h, LATER);
    let second = fixtures.check(
        "session_note",
        rpc.request(
            "session.note",
            json!({"op_id":"note-2","id":timer,"text":"Second thought"}),
        ),
    );
    assert_eq!(
        good(
            &mut rpc,
            "session.note",
            json!({"op_id":"note-2","text":"Replace?"})
        ),
        second
    );
    let row = &second["changed"]["sessions"][0];
    assert_eq!(row["notes"][0]["id"], first_id);
    assert_eq!(row["notes"][0]["at"], "2031-05-14T04:30:00Z");
    assert_eq!(row["notes"][1]["at"], "2031-05-14T04:55:00Z");
    assert_eq!(row["notes"][1]["text"], "Second thought");
    assert_eq!(row["running"], true);
    assert_eq!(
        good(&mut rpc, "status.get", json!({}))["running"]["notes"],
        row["notes"]
    );

    let stopped = good(
        &mut rpc,
        "session.stop",
        json!({"op_id":"stop","values":{"focus":4}}),
    );
    assert_eq!(stopped["changed"]["sessions"][0]["notes"], row["notes"]);
    let stats = good(&mut rpc, "stats.get", json!({"days":1}));
    let series = stats["series"].as_array().unwrap();
    assert_eq!(
        series
            .iter()
            .find(|s| s["metric_id"] == "distractions")
            .unwrap()["points"][0]["value"],
        1.0
    );
    assert_eq!(
        stopped["changed"]["sessions"][0]["values"],
        json!({"focus":4.0,"distractions":1.0})
    );
    bad(
        &mut rpc,
        "session.note",
        json!({"op_id":"late-note","id":timer,"text":"Late"}),
        "not_found",
        None,
    );
    bad(
        &mut rpc,
        "session.bump",
        json!({"op_id":"late-bump","id":timer,"metric_id":"distractions","delta":1}),
        "not_found",
        None,
    );
    bad(
        &mut rpc,
        "session.edit",
        json!({"op_id":"remove-first","id":timer,"remove_note_ids":[first_id,"unknown"]}),
        "validation",
        Some("remove_note_ids"),
    );
    let edited = fixtures.check("session_edit_remove_note", rpc.request("session.edit", json!({"op_id":"remove-first","id":timer,"remove_note_ids":[first_id],"note":"Updated summary"})));
    assert_eq!(
        edited["changed"]["sessions"][0]["notes"],
        json!([row["notes"][1]])
    );
    assert_eq!(edited["changed"]["sessions"][0]["note"], "Updated summary");
    good(
        &mut rpc,
        "session.remove",
        json!({"op_id":"remove","id":timer}),
    );
    let store = salah_store::Store::open(&h.data).unwrap();
    assert!(store.session_notes(&timer).unwrap().is_empty());
    let stats = good(&mut rpc, "stats.get", json!({"days":1}));
    assert_eq!(stats["totals"]["sessions"], 0);
}
