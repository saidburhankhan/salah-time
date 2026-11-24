//! Moving a task reclassifies its work, including sessions older than its epic.
use super::{
    DATE, Fixtures, Harness, NOW, Session,
    phase2::{added, bad, good},
};
use serde_json::{Value, json};

fn row<'a>(rows: &'a Value, field: &str, id: &str) -> &'a Value {
    rows.as_array()
        .unwrap()
        .iter()
        .find(|r| r[field] == id)
        .unwrap()
}

#[test]
fn ten_earlier_sessions_follow_the_task_across_reports_without_rewriting_history() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let old = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"old","name":"Old project"}),
    );
    let new = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"new","name":"Study"}),
    );
    let old_epic = added(
        &mut rpc,
        "epic.add",
        "epics",
        json!({"op_id":"before","name":"Other epic","project_id":old}),
    );
    let task = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"task","title":"Read","backlog":true}),
    );
    for i in 0..10 {
        if i == 6 {
            good(
                &mut rpc,
                "item.edit",
                json!({"id":task,"op_id":"first-move","project_id":old}),
            );
        }
        good(
            &mut rpc,
            "session.log",
            json!({"op_id":format!("s{i}"),"item_id":task,"minutes":3,"note":"Study","values":{"focus":4}}),
        );
    }
    let bare_epic = added(
        &mut rpc,
        "session.log",
        "sessions",
        json!({"op_id":"bare-epic","epic_id":old_epic,"minutes":7}),
    );
    let bare_project = added(
        &mut rpc,
        "session.log",
        "sessions",
        json!({"op_id":"bare-project","project_id":old,"minutes":5}),
    );
    let store = salah_store::Store::open(&h.data).unwrap();
    let day = DATE.parse().unwrap();
    let stored = store.sessions_between(day, day).unwrap();
    assert_eq!(
        stored
            .iter()
            .filter(|s| s.item_id.as_ref() == Some(&task) && s.project_id.is_none())
            .count(),
        6
    );

    let epic = added(
        &mut rpc,
        "epic.add",
        "epics",
        json!({"op_id":"bayyinah","name":"Bayyinah","project_id":new}),
    );
    let eg = added(
        &mut rpc,
        "goal.add",
        "goals",
        json!({"op_id":"epic-goal","period":"day","epic_id":epic,"target":30}),
    );
    let pg = added(
        &mut rpc,
        "goal.add",
        "goals",
        json!({"op_id":"project-goal","period":"week","project_id":new,"target":30}),
    );
    let before = good(&mut rpc, "goals.list", json!({}));
    assert_eq!(row(&before, "id", &eg)["progress"]["value"], 0.);
    good(
        &mut rpc,
        "item.edit",
        json!({"id":task,"op_id":"move","project_id":new,"epic_id":epic}),
    );
    assert_eq!(store.sessions_between(day, day).unwrap(), stored);

    let stats = Fixtures::default().check(
        "stats_epic_reassigned",
        rpc.request("stats.get", json!({"days":1,"epic_id":epic})),
    );
    assert_eq!(stats["totals"]["minutes"], 30);
    assert_eq!(stats["totals"]["sessions"], 10);
    assert_eq!(row(&stats["epics"], "id", &epic)["project_id"], new);
    assert_eq!(row(&stats["projects"], "id", &new)["minutes"], 30);
    for (dimension, id, minutes, count) in [
        ("project", &new, 30, 10),
        ("project", &old, 12, 2),
        ("epic", &epic, 30, 10),
        ("epic", &old_epic, 7, 1),
    ] {
        let table = good(&mut rpc, "stats.table", json!({"days":1,"group":dimension}));
        let bucket = row(&table["rows"], "key", id);
        assert_eq!(bucket["minutes"], minutes);
        assert_eq!(bucket["sessions"], count);
        let series = good(
            &mut rpc,
            "stats.series",
            json!({"days":1,"measures":["minutes"],"split":dimension}),
        );
        assert_eq!(
            row(&series["series"], "key", id)["points"][0]["value"],
            minutes as f64
        );
    }
    let no_project = good(
        &mut rpc,
        "stats.table",
        json!({"days":1,"project_id":"none"}),
    );
    assert_eq!(no_project["totals"]["sessions"], 0);
    let no_epic = good(
        &mut rpc,
        "sessions.list",
        json!({"days":1,"epic_id":"none","with_note":false}),
    );
    assert_eq!(no_epic["rows"].as_array().unwrap().len(), 1);
    assert_eq!(no_epic["rows"][0]["id"], bare_project);
    let bare = good(
        &mut rpc,
        "sessions.list",
        json!({"days":1,"item_id":"none","project_id":old,"with_note":false}),
    );
    assert_eq!(row(&bare["rows"], "id", &bare_epic)["epic"]["id"], old_epic);
    let list = good(
        &mut rpc,
        "sessions.list",
        json!({"days":1,"epic_id":epic,"project_id":new}),
    );
    assert_eq!(list["rows"].as_array().unwrap().len(), 10);
    for r in list["rows"].as_array().unwrap() {
        assert_eq!(r["project_id"], new);
        assert_eq!(r["epic_id"], epic);
        assert_eq!(r["item_id"], task);
        assert_eq!(r["project"]["name"], "Study");
        assert_eq!(r["epic"]["name"], "Bayyinah");
    }
    let insights = good(
        &mut rpc,
        "insights.get",
        json!({"days":1,"project_id":new,"epic_id":epic}),
    );
    assert_eq!(insights["by_weekday"][2]["minutes"], 30);
    assert_eq!(insights["best"][0]["epic_id"], epic);
    let review = good(&mut rpc, "review.get", json!({"week":"2031-05-12"}));
    let summary = &review["summary"];
    assert_eq!(row(&summary["by_project"], "id", &old)["minutes"], 12);
    assert_eq!(row(&summary["by_epic"], "id", &epic)["minutes"], 30);
    assert_eq!(summary["sessions_with_notes"][0]["project_id"], new);
    let goals = good(&mut rpc, "goals.list", json!({}));
    for g in [&eg, &pg] {
        assert_eq!(row(&goals, "id", g)["progress"]["value"], 30.);
        assert_eq!(
            row(&summary["goals_met"], "id", g)["progress"]["value"],
            30.
        );
        let history = good(
            &mut rpc,
            "goals.history",
            json!({"id":g,"from":DATE,"to":DATE}),
        );
        assert_eq!(history[0]["value"], 30.);
    }
    let epics = good(&mut rpc, "epics.list", json!({"project_id":new}));
    let view = row(&epics, "id", &epic);
    assert_eq!(view["sessions"], 10);
    assert_eq!(view["total_minutes"], 30);
    assert_eq!(view["open_tasks"], 1);
    assert_eq!(
        good(&mut rpc, "backlog.list", json!({"project_id":new}))[0]["id"],
        task
    );
    bad(
        &mut rpc,
        "epic.remove",
        json!({"op_id":"busy","id":epic}),
        "conflict",
        None,
    );
    assert_eq!(store.sessions_between(day, day).unwrap(), stored);

    // A current null wins even when the session was recorded with a project.
    good(
        &mut rpc,
        "item.edit",
        json!({"op_id":"detach","id":task,"project_id":null}),
    );
    let loose = good(
        &mut rpc,
        "stats.table",
        json!({"days":1,"project_id":"none","epic_id":"none"}),
    );
    assert_eq!(loose["totals"]["sessions"], 10);
    assert_eq!(loose["totals"]["minutes"], 30);
    let epics = good(&mut rpc, "epics.list", json!({}));
    assert_eq!(row(&epics, "id", &old_epic)["total_minutes"], 7);
    assert_eq!(row(&epics, "id", &epic)["sessions"], 0);
    good(&mut rpc, "epic.remove", json!({"op_id":"empty","id":epic}));
    assert_eq!(store.sessions_between(day, day).unwrap(), stored);
}
