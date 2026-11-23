use super::phase2::{added, bad, good};
use super::{DATE, Fixtures, Harness, LATER, NOW, Session, id};
use serde_json::json;

fn project(rpc: &mut Session, name: &str) -> String {
    added(
        rpc,
        "project.add",
        "projects",
        json!({"op_id":name,"name":name}),
    )
}

#[test]
fn epic_lifecycle_keeps_versions_history_and_replays() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let mut fixtures = Fixtures::default();
    let p = project(&mut rpc, "Library");
    let other = project(&mut rpc, "Other");
    let created = fixtures.check(
        "epic_add",
        rpc.request(
            "epic.add",
            json!({"op_id":"e","project_id":p,"name":"Read","note":"Spring"}),
        ),
    );
    let e = id(&created, "epics");
    assert_eq!(good(&mut rpc, "epic.add", json!({"op_id":"e"})), created);
    let routine = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"routine","title":"Morning reading","repeat":"daily","planned_minutes":20,"block":"fajr","project_id":p,"epic_id":e}),
    );
    bad(
        &mut rpc,
        "item.add",
        json!({"op_id":"invalid-item","title":"Wrong owner","project_id":other,"epic_id":e}),
        "validation",
        Some("epic_id"),
    );
    bad(
        &mut rpc,
        "item.add",
        json!({"op_id":"no-project","title":"Wrong owner","project_id":null,"epic_id":"missing"}),
        "not_found",
        None,
    );
    bad(
        &mut rpc,
        "session.start",
        json!({"op_id":"mismatch","project_id":other,"epic_id":e}),
        "validation",
        Some("project_id"),
    );
    let log = fixtures.check("session_log_epic",rpc.request("session.log",json!({"op_id":"log","epic_id":e,"date":"2031-05-13","minutes":30,"values":{"focus":4}})));
    let session = &log["changed"]["sessions"][0];
    assert_eq!(session["project_id"], p);
    assert_eq!(session["epic_id"], e);
    assert!(session["item_id"].is_null());
    let tick = added(
        &mut rpc,
        "session.log",
        "sessions",
        json!({"op_id":"tick","item_id":routine}),
    );
    let started = good(
        &mut rpc,
        "session.start",
        json!({"op_id":"timer","epic_id":e}),
    );
    assert_eq!(started["changed"]["sessions"][0]["project_id"], p);
    let listed = fixtures.check(
        "epics_list",
        rpc.request("epics.list", json!({"status":"active","project_id":p})),
    );
    assert_eq!(listed[0]["sessions"], 2);
    assert_eq!(listed[0]["total_minutes"], 30);
    assert_eq!(listed[0]["days_worked"], 2);
    assert_eq!(listed[0]["last_worked"], DATE);
    assert_eq!(listed[0]["done_tasks"], 1);
    assert_eq!(listed[0]["open_tasks"], 0);
    good(
        &mut rpc,
        "session.remove",
        json!({"op_id":"untick","id":tick}),
    );
    let unticked = good(&mut rpc, "epics.list", json!({}));
    assert_eq!(unticked[0]["open_tasks"], 1);
    assert_eq!(unticked[0]["sessions"], 1);
    let finished = fixtures.check("epic_edit_finish",rpc.request("epic.edit",json!({"op_id":"finish","id":e,"expected_version":1,"name":"Read in spring","status":"finished","note":null})));
    let finished_at = &finished["changed"]["epics"][0]["finished_at"];
    assert!(finished_at.is_string());
    // A finished epic takes late sessions but no new tasks.
    let finished_msg = |response: &serde_json::Value| {
        assert_eq!(
            response["error"]["data"]["kind"], "validation",
            "{response}"
        );
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap()
                .contains("epic is finished; reopen it to add tasks"),
            "{response}"
        );
    };
    finished_msg(&rpc.request(
        "item.add",
        json!({"op_id":"late-task","title":"Late","project_id":p,"epic_id":e}),
    ));
    let loose = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"loose","title":"Loose","project_id":p}),
    );
    finished_msg(&rpc.request(
        "item.edit",
        json!({"op_id":"into-finished","id":loose,"epic_id":e}),
    ));
    good(
        &mut rpc,
        "item.edit",
        json!({"op_id":"keep-finished","id":routine,"title":"Morning pages"}),
    );
    let late = good(
        &mut rpc,
        "session.log",
        json!({"op_id":"late-log","epic_id":e,"date":DATE,"minutes":5}),
    );
    assert_eq!(late["changed"]["sessions"][0]["epic_id"], e);
    good(
        &mut rpc,
        "item.remove",
        json!({"op_id":"drop-loose","id":loose}),
    );
    assert_eq!(
        good(&mut rpc, "epics.list", json!({"status":"active"})),
        json!([])
    );
    bad(
        &mut rpc,
        "epic.edit",
        json!({"op_id":"stale","id":e,"expected_version":1,"status":"active"}),
        "conflict",
        None,
    );
    drop(rpc);
    let mut later = Session::open(&h, LATER);
    let again = good(
        &mut later,
        "epic.edit",
        json!({"op_id":"again","id":e,"status":"finished"}),
    );
    assert_eq!(again["changed"]["epics"][0]["finished_at"], *finished_at);
    let reopened = good(
        &mut later,
        "epic.edit",
        json!({"op_id":"reopen","id":e,"status":"active"}),
    );
    assert!(reopened["changed"]["epics"][0]["finished_at"].is_null());
    bad(
        &mut later,
        "epic.remove",
        json!({"op_id":"remove","id":e}),
        "conflict",
        None,
    );
    let stopped = good(&mut later, "session.stop", json!({"op_id":"stop"}));
    assert_eq!(stopped["changed"]["sessions"][0]["epic_id"], e);
    let free = added(
        &mut later,
        "epic.add",
        "epics",
        json!({"op_id":"empty","project_id":p,"name":"Unused"}),
    );
    good(
        &mut later,
        "item.edit",
        json!({"op_id":"assign","id":routine,"epic_id":free}),
    );
    let removed = good(
        &mut later,
        "epic.remove",
        json!({"op_id":"delete","id":free}),
    );
    assert_eq!(removed["changed"]["deleted"], json!([free]));
    assert_eq!(removed["changed"]["items"][0]["id"], routine);
    assert!(removed["changed"]["items"][0]["epic_id"].is_null());
    assert_eq!(
        good(&mut later, "epic.remove", json!({"op_id":"delete"})),
        removed
    );
    bad(
        &mut later,
        "epic.edit",
        json!({"op_id":"gone","id":free}),
        "not_found",
        None,
    );
}

#[test]
fn epic_reports_use_recorded_ownership_and_include_unassigned_work() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let mut fixtures = Fixtures::default();
    let p = project(&mut rpc, "Library");
    let e = added(
        &mut rpc,
        "epic.add",
        "epics",
        json!({"op_id":"epic","project_id":p,"name":"Reading"}),
    );
    let item = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"item","title":"Read","project_id":p,"epic_id":e,"planned_minutes":30,"repeat":"daily"}),
    );
    let unassigned = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"walk","title":"Walk"}),
    );
    let timer = good(
        &mut rpc,
        "session.start",
        json!({"op_id":"start","item_id":item}),
    );
    assert_eq!(timer["changed"]["sessions"][0]["epic_id"], e);
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"project-log","project_id":p,"minutes":12}),
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"walk-log","item_id":unassigned,"minutes":8}),
    );
    drop(rpc);
    let mut rpc = Session::open(&h, LATER);
    let stopped = good(
        &mut rpc,
        "session.stop",
        json!({"op_id":"stop","values":{"focus":4},"done":true}),
    );
    for session in stopped["changed"]["sessions"].as_array().unwrap() {
        assert_eq!(session["epic_id"], e);
    }
    let table = fixtures.check(
        "stats_table_epic",
        rpc.request("stats.table", json!({"days":1,"group":"epic"})),
    );
    assert_eq!(table["totals"]["minutes"], 45);
    assert_eq!(table["rows"][0]["key"], e);
    assert_eq!(table["rows"][0]["sessions"], 2);
    assert_eq!(table["rows"][1]["key"], "none");
    assert_eq!(table["rows"][1]["label"], "No epic");
    assert_eq!(table["rows"][1]["minutes"], 20);

    let stats = good(&mut rpc, "stats.get", json!({"days":1}));
    let no_project = stats["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "none")
        .unwrap();
    assert_eq!(no_project["name"], "No project");
    assert_eq!(no_project["minutes"], 8);
    let groups = stats["epics"].as_array().unwrap();
    // Project order, its epics before its "No epic" row, work without a project last.
    let order: Vec<_> = groups
        .iter()
        .map(|r| (&r["project_id"], &r["id"]))
        .collect();
    let (p_id, e_id, none) = (json!(p), json!(e), json!("none"));
    assert_eq!(
        order,
        [
            (&p_id, &e_id),
            (&p_id, &none),
            (&serde_json::Value::Null, &none)
        ]
    );
    assert_eq!(
        groups
            .iter()
            .map(|r| r["minutes"].as_i64().unwrap())
            .sum::<i64>(),
        45
    );
    let assigned = groups.iter().find(|r| r["id"] == e).unwrap();
    assert_eq!(assigned["metrics"]["focus"], 4.0);
    let gap = groups
        .iter()
        .find(|r| r["id"] == "none" && r["project_id"] == p)
        .unwrap();
    assert_eq!(gap["minutes"], 12);
    let trend = good(
        &mut rpc,
        "stats.series",
        json!({"days":1,"split":"epic","measures":["minutes"]}),
    );
    assert_eq!(trend["series"][0]["key"], e);
    assert_eq!(trend["series"][0]["points"][0]["value"], 25.0);
    assert_eq!(trend["series"][1]["key"], "none");
    assert_eq!(trend["series"][1]["points"][0]["value"], 20.0);
    let filter =
        json!({"days":1,"epic_id":e,"project_id":p,"minutes_min":10,"values":{"focus":{"min":4}}});
    for method in ["stats.get", "stats.table"] {
        let stats = good(&mut rpc, method, filter.clone());
        assert_eq!(stats["totals"]["minutes"], 25);
        assert_eq!(stats["totals"]["sessions"], 1);
        assert_eq!(stats["filter"]["epic_id"], e);
    }
    let mut query = filter;
    query["with_note"] = json!(false);
    assert_eq!(
        good(&mut rpc, "sessions.list", query)["rows"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    for (project_id, minutes) in [(json!(p), 12), (json!("none"), 8)] {
        let filter = json!({"days":1,"epic_id":"none","project_id":project_id,"with_note":false});
        assert_eq!(
            good(&mut rpc, "stats.get", filter.clone())["totals"]["minutes"],
            minutes
        );
        assert_eq!(
            good(&mut rpc, "stats.table", filter.clone())["totals"]["minutes"],
            minutes
        );
        let rows = good(&mut rpc, "sessions.list", filter)["rows"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["minutes"], minutes);
        assert!(rows[0]["epic"].is_null());
    }
    drop(rpc);
    let mut next = Session::open(&h, "2031-05-15T10:00:00+05:00");
    let moved = good(
        &mut next,
        "item.edit",
        json!({"op_id":"move-project","id":item,"project_id":null}),
    );
    assert!(moved["changed"]["items"][0]["epic_id"].is_null());
    let old = good(
        &mut next,
        "stats.get",
        json!({"from":DATE,"to":DATE,"epic_id":e}),
    );
    assert_eq!(old["days"][0]["planned_minutes"], 30);
    assert_eq!(old["totals"]["minutes"], 25);
    let today = good(&mut next, "stats.get", json!({"days":1,"epic_id":e}));
    assert_eq!(today["days"][0]["planned_minutes"], 0);
    let sessions = good(
        &mut next,
        "sessions.list",
        json!({"days":2,"epic_id":e,"with_note":false}),
    );
    assert_eq!(sessions["rows"].as_array().unwrap().len(), 2);
    assert!(
        sessions["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["epic_id"] == e
                && s["project_id"] == p
                && s["epic"] == json!({"id": e, "name": "Reading"}))
    );
    bad(
        &mut next,
        "session.log",
        json!({"op_id":"wrong-epic","item_id":item,"epic_id":e}),
        "validation",
        Some("epic_id"),
    );
}

#[test]
fn an_item_with_only_an_epic_joins_the_epic_project() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let p = project(&mut rpc, "Library");
    let e = added(
        &mut rpc,
        "epic.add",
        "epics",
        json!({"op_id":"epic","project_id":p,"name":"Reading"}),
    );
    let added_item = good(
        &mut rpc,
        "item.add",
        json!({"op_id":"outline","title":"Outline","epic_id":e}),
    );
    let item = &added_item["changed"]["items"][0];
    assert_eq!(
        (&item["project_id"], &item["epic_id"]),
        (&json!(p), &json!(e))
    );
    let loose = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"loose","title":"Loose"}),
    );
    let joined = good(
        &mut rpc,
        "item.edit",
        json!({"op_id":"join","id":loose,"epic_id":e}),
    );
    assert_eq!(joined["changed"]["items"][0]["project_id"], p);
    let other = project(&mut rpc, "Other");
    bad(
        &mut rpc,
        "item.add",
        json!({"op_id":"mismatch","title":"X","project_id":other,"epic_id":e}),
        "validation",
        Some("epic_id"),
    );
}
