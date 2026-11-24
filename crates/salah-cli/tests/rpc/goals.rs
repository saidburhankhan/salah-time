use super::{
    DATE, Fixtures, Harness, NOW, Session,
    phase2::{added, bad, good},
};
use serde_json::{Value, json};

fn create(rpc: &mut Session, op: &str, scope: Value, period: &str, target: f64) -> String {
    let mut p = scope;
    p["op_id"] = json!(op);
    p["period"] = json!(period);
    p["target"] = json!(target);
    added(rpc, "goal.add", "goals", p)
}
fn goal<'a>(list: &'a Value, id: &str) -> &'a Value {
    list.as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == id)
        .unwrap()
}

#[test]
fn goal_progress_uses_session_scope_and_stats_weights() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let project = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"project","name":"Study"}),
    );
    let epic = added(
        &mut rpc,
        "epic.add",
        "epics",
        json!({"op_id":"epic","project_id":project,"name":"Book"}),
    );
    let task = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"task","epic_id":epic,"title":"Read"}),
    );
    let loose = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"free","title":"Walk"}),
    );
    let mut fixtures = Fixtures::default();
    let result = fixtures.check(
        "goal_add",
        rpc.request(
            "goal.add",
            json!({"op_id":"all","period":"day","target":60}),
        ),
    );
    let all = result["changed"]["goals"][0]["id"].as_str().unwrap();
    let p = create(&mut rpc, "p", json!({"project_id":project}), "day", 50.0);
    let e = create(&mut rpc, "e", json!({"epic_id":epic}), "week", 40.0);
    let t = create(&mut rpc, "t", json!({"item_id":task}), "day", 30.0);
    let focus = create(&mut rpc, "focus", json!({"metric_id":"focus"}), "week", 4.0);
    let count = create(
        &mut rpc,
        "count",
        json!({"project_id":project,"metric_id":"distractions","direction":"at_most"}),
        "week",
        3.0,
    );
    let empty = good(&mut rpc, "goals.list", json!({}));
    assert_eq!(goal(&empty, &focus)["progress"]["value"], Value::Null);
    assert_eq!(goal(&empty, &count)["progress"]["met"], false);
    // One instant per write keeps the day's session order (and the fixture)
    // stable: sessions logged at the same instant tie on their random ids.
    for (minute, (op, params)) in [
        (
            "read",
            json!({"item_id":task,"minutes":30,"values":{"focus":5,"distractions":1}}),
        ),
        (
            "tick",
            json!({"item_id":task,"values":{"focus":1,"distractions":1}}),
        ),
        (
            "epic-log",
            json!({"epic_id":epic,"minutes":10,"values":{"focus":3}}),
        ),
        (
            "project-log",
            json!({"project_id":project,"minutes":10,"values":{"distractions":1}}),
        ),
        (
            "walk",
            json!({"item_id":loose,"minutes":12,"values":{"distractions":5}}),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut params = params;
        params["op_id"] = json!(op);
        rpc = Session::open(&h, &format!("2031-05-14T09:3{}:00+05:00", minute + 1));
        good(&mut rpc, "session.log", params);
    }
    rpc = Session::open(&h, "2031-05-14T09:40:00+05:00");
    good(
        &mut rpc,
        "session.start",
        json!({"op_id":"timer","item_id":task}),
    );
    let list = fixtures.check("goals_list", rpc.request("goals.list", json!({})));
    for (id, v) in [
        (all, 62.0),
        (&p, 50.0),
        (&e, 40.0),
        (&t, 30.0),
        (&count, 3.0),
    ] {
        let progress = &goal(&list, id)["progress"];
        assert_eq!(progress["value"], v);
        assert_eq!(progress["met"], true);
    }
    assert!(
        (goal(&list, &focus)["progress"]["value"].as_f64().unwrap() - 181.0 / 41.0).abs() < 1e-9
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"over","project_id":project,"values":{"distractions":1}}),
    );
    assert_eq!(
        goal(&good(&mut rpc, "goals.list", json!({})), &count)["progress"]["met"],
        false
    );
    let status = fixtures.check("status_goals", rpc.request("status.get", json!({})));
    assert_eq!(status["goals"], good(&mut rpc, "goals.list", json!({})));
}

#[test]
fn sleep_rolls_over_both_sunday_periods_and_edits_preserve_history() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, "2031-05-18T22:59:00+05:00");
    let task = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"t","title":"Walk"}),
    );
    let daily = create(&mut rpc, "day", json!({}), "day", 15.0);
    let weekly = create(&mut rpc, "week", json!({}), "week", 50.0);
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"sun","item_id":task,"minutes":10}),
    );
    assert_eq!(
        goal(&good(&mut rpc, "goals.list", json!({})), &weekly)["progress"]["from"],
        "2031-05-12"
    );
    drop(rpc);
    let mut rpc = Session::open(&h, "2031-05-18T23:00:00+05:00");
    let list = good(&mut rpc, "goals.list", json!({}));
    for id in [&weekly, &daily] {
        assert_eq!(goal(&list, id)["progress"]["from"], "2031-05-19");
        assert_eq!(goal(&list, id)["progress"]["value"], 0.0);
    }
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"mon","item_id":task,"minutes":60}),
    );
    drop(rpc);
    let mut rpc = Session::open(&h, "2031-05-21T12:00:00+05:00");
    let edit = json!({"op_id":"edit","id":weekly,"target":55});
    let mut fixtures = Fixtures::with_ids(&[&weekly]);
    let edited = fixtures.check(
        "goal_edit_successor",
        rpc.request("goal.edit", edit.clone()),
    );
    let new = &edited["changed"]["goals"][0];
    assert_eq!(new["replaces_id"], weekly);
    assert_eq!(new["starts_on"], "2031-05-19");
    assert_eq!(edited["changed"]["goals"][1]["ends_on"], "2031-05-18");
    assert_eq!(good(&mut rpc, "goal.edit", edit), edited);
    let params = json!({"id":new["id"],"from":"2031-05-01","to":"2031-05-31"});
    let history = fixtures.check("goals_history", rpc.request("goals.history", params));
    assert_eq!(
        history,
        json!([
            {"from":"2031-05-12","to":"2031-05-18","value":10.0,"target":50.0,"met":false},
            {"from":"2031-05-19","to":"2031-05-25","value":60.0,"target":55.0,"met":true}
        ])
    );
    let old = good(&mut rpc, "goals.list", json!({"at":"2031-05-18"}));
    assert_eq!(goal(&old, &weekly)["target"], 50.0);
    // Revise again within the successor's first period. It has no completed history.
    let revised = good(
        &mut rpc,
        "goal.edit",
        json!({"id":new["id"],"target":70,"op_id":"again"}),
    );
    assert_eq!(revised["changed"]["deleted"], json!([new["id"]]));
    let history = good(
        &mut rpc,
        "goals.history",
        json!({"id":weekly,"from":"2031-05-01","to":"2031-05-31"}),
    );
    assert_eq!(history.as_array().unwrap().len(), 2);
    assert_eq!(history[1]["target"], 70.0);
    bad(
        &mut rpc,
        "goal.remove",
        json!({"op_id":"old","id":daily}),
        "validation",
        Some("id"),
    );
    good(&mut rpc, "goal.end", json!({"op_id":"end","id":daily}));
    let ended = good(&mut rpc, "goals.list", json!({"include_ended":true}));
    assert_eq!(goal(&ended, &daily)["ends_on"], "2031-05-20");
}

#[test]
fn validates_goal_target_scope_and_calendar_and_ends_removed_scopes() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let p = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"p","name":"Project","metrics":[]}),
    );
    let e = added(
        &mut rpc,
        "epic.add",
        "epics",
        json!({"op_id":"e","project_id":p,"name":"Epic"}),
    );
    let t = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"t","title":"Read","project_id":p,"metric_ids":["focus"]}),
    );
    let metric = added(
        &mut rpc,
        "metric.add",
        "metrics",
        json!({"op_id":"custom","kind":"count","name":"Pages"}),
    );
    for (i, extra) in [
        json!({"target":-3}),
        json!({"target":0}),
        json!({"target":"60"}),
        json!({"direction":"at_most"}),
        json!({"project_id":p,"item_id":t}),
        json!({"metric_id":metric}),
        json!({"epic_id":e,"metric_id":"focus"}),
        json!({"project_id":p,"metric_id":"focus"}),
        json!({"item_id":t,"metric_id":"quality"}),
        json!({"starts_on":"2031-02-30"}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut body = json!({"op_id":format!("invalid-{i}"),"period":"week","target":20});
        body.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert!(
            rpc.request("goal.add", body).get("error").is_some(),
            "{extra}"
        );
    }
    let tg = create(
        &mut rpc,
        "tg",
        json!({"item_id":t,"metric_id":"focus"}),
        "day",
        4.0,
    );
    let eg = create(&mut rpc, "eg", json!({"epic_id":e}), "week", 20.0);
    let removed = create(&mut rpc, "temp", json!({}), "day", 10.0);
    good(&mut rpc, "goal.remove", json!({"id":removed,"op_id":"del"}));
    let invalid = rpc.request("goal.edit", json!({"id":tg,"op_id":"negative","target":-1}));
    assert!(invalid.get("error").is_some());
    assert_eq!(
        good(&mut rpc, "goals.list", json!({}))
            .as_array()
            .unwrap()
            .len(),
        2
    );
    for at in ["2031-5-14", "2031-02-29", "+12031-05-14"] {
        bad(
            &mut rpc,
            "goals.list",
            json!({"at":at}),
            "validation",
            Some("at"),
        );
        bad(
            &mut rpc,
            "goals.history",
            json!({"id":tg,"from":DATE,"to":at}),
            "validation",
            Some("to"),
        );
    }
    bad(
        &mut rpc,
        "goals.history",
        json!({"id":tg,"from":"2029-05-14","to":DATE}),
        "validation",
        Some("to"),
    );
    drop(rpc);
    let mut rpc = Session::open(&h, "2031-05-23T09:00:00+05:00");
    good(
        &mut rpc,
        "item.edit",
        json!({"id":t,"op_id":"rename","title":"Read notes"}),
    );
    let dropped = good(&mut rpc, "item.remove", json!({"id":t,"op_id":"drop-t"}));
    assert_eq!(dropped["changed"]["goals"][0]["id"], tg);
    assert_eq!(dropped["changed"]["goals"][0]["scope_label"], "Read notes");
    assert_eq!(dropped["changed"]["deleted"], json!([t]));
    let dropped = good(&mut rpc, "epic.remove", json!({"id":e,"op_id":"drop-e"}));
    assert_eq!(dropped["changed"]["goals"][0]["ends_on"], "2031-05-18");
    assert_eq!(good(&mut rpc, "goals.list", json!({})), json!([]));
    let ended = good(&mut rpc, "goals.list", json!({"include_ended":true}));
    assert_eq!(goal(&ended, &tg)["scope_label"], "Read notes");
    assert_eq!(goal(&ended, &eg)["scope_label"], "Epic");
    assert_eq!(goal(&ended, &tg)["ends_on"], "2031-05-22");
    assert_eq!(goal(&ended, &eg)["ends_on"], "2031-05-18");
}

#[test]
fn weighted_equality_meets_both_directions_in_status_history_and_review() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let p = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"p","name":"Study"}),
    );
    for (rating, minutes) in [(1, 1), (2, 11), (3, 1)] {
        good(
            &mut rpc,
            "session.log",
            json!({"op_id":format!("s{rating}"),"project_id":p,"minutes":minutes,"values":{"focus":rating}}),
        );
    }
    for direction in ["at_least", "at_most"] {
        let id = create(
            &mut rpc,
            direction,
            json!({"direction":direction,"metric_id":"focus"}),
            "week",
            2.,
        );
        let status = good(&mut rpc, "status.get", json!({}));
        let progress = &goal(&status["goals"], &id)["progress"];
        assert_eq!(progress["value"], 2.);
        assert_eq!(progress["met"], true);
        let history = good(
            &mut rpc,
            "goals.history",
            json!({"id":id,"from":DATE,"to":DATE}),
        );
        assert_eq!(history[0]["met"], true);
        let review = good(&mut rpc, "review.get", json!({"week":"2031-05-12"}));
        assert_eq!(
            goal(&review["summary"]["goals_met"], &id)["progress"],
            *progress
        );
    }
}
