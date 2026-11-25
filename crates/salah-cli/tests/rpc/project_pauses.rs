use super::{
    Fixtures, Harness, Session,
    phase2::{added, bad, good},
};
use serde_json::{Value, json};

const MON: &str = "2031-05-12";
const TUE: &str = "2031-05-13";
const WED: &str = "2031-05-14";
const THU: &str = "2031-05-15";
const FRI: &str = "2031-05-16";
fn on(h: &Harness, date: &str) -> Session {
    Session::open(h, &format!("{date}T09:30:00+05:00"))
}
fn archive(r: &mut Session, p: &str, op: &str, flag: bool) -> Value {
    good(
        r,
        "project.edit",
        json!({"op_id":op,"id":p,"archived":flag}),
    )
}
fn task(r: &mut Session, p: &str, name: &str, fields: Value) -> String {
    let mut params = fields;
    params["op_id"] = json!(name);
    params["title"] = json!(name);
    params["project_id"] = json!(p);
    added(r, "item.add", "items", params)
}
fn day_items(day: &Value) -> Vec<&Value> {
    day["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|b| b["items"].as_array().unwrap())
        .collect()
}
fn named<'a>(rows: &'a Value, id: &str) -> &'a Value {
    rows.as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap()
}

#[test]
fn three_day_archive_preserves_work_and_recovers_only_missed_open_tasks() {
    let h = Harness::new();
    let mut r = on(&h, MON);
    let p = added(
        &mut r,
        "project.add",
        "projects",
        json!({"op_id":"p","name":"Study"}),
    );
    let epic = added(
        &mut r,
        "epic.add",
        "epics",
        json!({"op_id":"e","name":"Course","project_id":p}),
    );
    let routine = task(
        &mut r,
        &p,
        "Daily",
        json!({"repeat":"daily","block":"fajr","planned_minutes":20,"epic_id":epic}),
    );
    let missed = task(&mut r, &p, "Wednesday", json!({"date":WED}));
    let completed = task(
        &mut r,
        &p,
        "Finished",
        json!({"date":TUE,"planned_minutes":40}),
    );
    let future = task(&mut r, &p, "Friday", json!({"date":FRI}));
    let backlog = task(&mut r, &p, "Later", json!({"backlog":true}));
    good(
        &mut r,
        "session.log",
        json!({"op_id":"mon","item_id":routine,"minutes":17,"block":"fajr"}),
    );
    good(
        &mut r,
        "session.log",
        json!({"op_id":"mon-done","item_id":routine}),
    );
    drop(r);
    let mut r = on(&h, TUE);
    good(
        &mut r,
        "session.log",
        json!({"op_id":"tue","item_id":routine,"minutes":11,"block":"fajr","note":"Logged"}),
    );
    good(
        &mut r,
        "session.log",
        json!({"op_id":"completed","item_id":completed}),
    );
    let range = json!({"from":MON,"to":FRI,"project_id":p});
    let totals = good(&mut r, "stats.table", range.clone())["totals"].clone();
    let before = good(&mut r, "day.get", json!({"date":MON}));
    assert_eq!(day_items(&before)[0]["actual_minutes"], 17);
    let preview = Fixtures::default().check(
        "project_pause_preview",
        r.request("project.pause_preview", json!({"id":p})),
    );
    assert_eq!(preview, json!({"routines":1,"open_tasks":3,"goals":0}));
    archive(&mut r, &p, "archive", true);
    assert_eq!(good(&mut r, "stats.table", range.clone())["totals"], totals);
    assert_eq!(good(&mut r, "day.get", json!({"date":MON})), before);
    assert_eq!(good(&mut r, "items.list", json!({})), json!([]));
    assert_eq!(good(&mut r, "epics.list", json!({})), json!([]));
    assert_eq!(good(&mut r, "backlog.list", json!({})), json!([]));
    let mut fixtures =
        Fixtures::with_ids(&[&p, &epic, &routine, &missed, &completed, &future, &backlog]);
    let catalog = fixtures.check(
        "items_list_paused",
        r.request("items.list", json!({"include_paused":true})),
    );
    assert!(
        catalog
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["paused"] == true && i["next_occurrence"].is_null())
    );
    assert_eq!(catalog.as_array().unwrap().len(), 5);
    assert_eq!(
        named(
            &fixtures.check(
                "epics_list_paused",
                r.request("epics.list", json!({"include_paused":true}))
            ),
            &epic
        )["paused"],
        true
    );
    assert_eq!(
        good(&mut r, "projects.list", json!({}))[0]["archived"],
        true
    );
    assert!(
        good(&mut r, "status.get", json!({}))["planner"]["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let day = good(&mut r, "day.get", json!({"date":TUE}));
    let rows = day_items(&day);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], completed);
    assert_eq!(rows[0]["done"], true);
    assert_eq!(day["fulfilment"]["planned_items"], 0);
    assert_eq!(day["fulfilment"]["unplanned_minutes"], 11);
    let insights = good(&mut r, "insights.get", range.clone());
    assert_eq!(insights["on_plan"][0]["on_plan_minutes"], 17);
    assert_eq!(insights["on_plan"][0]["minutes"], 28);
    assert_eq!(insights["on_plan"][0]["scored_minutes"], 17);
    assert_eq!(
        good(&mut r, "routines.activity", json!({"from":MON,"to":TUE}))["rows"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let notes = good(&mut r, "sessions.list", range.clone());
    assert_eq!(notes["rows"][0]["note"], "Logged");
    drop(r);
    let mut r = on(&h, FRI);
    // Wednesday gets frozen while paused, Thursday is first opened after resume.
    assert!(day_items(&good(&mut r, "day.get", json!({"date":WED}))).is_empty());
    let store = salah_store::Store::open(&h.data).unwrap();
    assert!(store.day_plan(WED.parse().unwrap()).unwrap().is_empty());
    bad(
        &mut r,
        "project.edit",
        json!({"id":p,"op_id":"invalid-resume","archived":false,"metrics":[{"metric_id":"unknown","required":false}]}),
        "not_found",
        None,
    );
    assert!(store.project(&p).unwrap().unwrap().archived);
    assert_eq!(
        store.item(&missed).unwrap().unwrap().day,
        Some(WED.parse().unwrap())
    );
    assert!(
        store
            .paused_projects_on(FRI.parse().unwrap())
            .unwrap()
            .contains(&p)
    );
    let changed = archive(&mut r, &p, "resume", false);
    assert_eq!(changed["changed"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(changed["changed"]["items"][0]["id"], missed);
    assert_eq!(changed["changed"]["items"][0]["backlog"], true);
    assert_eq!(archive(&mut r, &p, "resume", false), changed);
    for day in [WED, THU] {
        assert!(day_items(&good(&mut r, "day.get", json!({"date":day}))).is_empty());
    }
    let planned = good(&mut r, "day.get", json!({"date":FRI}));
    let rows = day_items(&planned);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|i| i["id"] == future));
    assert!(rows.iter().any(|i| i["id"] == routine));
    let recovered = good(&mut r, "backlog.list", json!({}));
    assert_eq!(recovered.as_array().unwrap().len(), 2);
    for id in [missed, backlog] {
        assert_eq!(named(&recovered, &id)["paused"], false);
    }
    assert_eq!(good(&mut r, "stats.table", range)["totals"], totals);
}

#[test]
fn overlapping_goal_periods_are_neutral_for_all_scoped_goals_and_review() {
    let h = Harness::new();
    let mut r = on(&h, MON);
    let p = added(
        &mut r,
        "project.add",
        "projects",
        json!({"op_id":"p","name":"Study"}),
    );
    let epic = added(
        &mut r,
        "epic.add",
        "epics",
        json!({"op_id":"e","name":"Course","project_id":p}),
    );
    let routine = task(
        &mut r,
        &p,
        "Daily",
        json!({"epic_id":epic,"repeat":"daily"}),
    );
    let mut goals = Vec::new();
    for (op, period, mut scope) in [
        ("project", "day", json!({"project_id":p})),
        ("epic", "week", json!({"epic_id":epic})),
        ("task", "day", json!({"item_id":routine})),
        ("all", "week", json!({})),
    ] {
        scope["op_id"] = json!(op);
        scope["period"] = json!(period);
        scope["target"] = json!(10);
        goals.push(added(&mut r, "goal.add", "goals", scope));
    }
    for (i, day) in [MON, TUE, FRI].iter().enumerate() {
        good(
            &mut r,
            "session.log",
            json!({"op_id":format!("work{i}"),"item_id":routine,"date":day,"minutes":10}),
        );
    }
    assert_eq!(
        good(&mut r, "project.pause_preview", json!({"id":p}))["goals"],
        3
    );
    archive(&mut r, &p, "same-archive", true);
    archive(&mut r, &p, "undo", false);
    assert!(
        good(&mut r, "goals.list", json!({}))
            .as_array()
            .unwrap()
            .iter()
            .all(|g| g["paused"] == false)
    );
    drop(r);
    let mut r = on(&h, TUE);
    archive(&mut r, &p, "archive", true);
    bad(
        &mut r,
        "project.edit",
        json!({"op_id":"stale","id":p,"archived":false,"expected_version":1}),
        "conflict",
        None,
    );
    let mut fixtures = Fixtures::with_ids(&[
        &p, &epic, &routine, &goals[0], &goals[1], &goals[2], &goals[3],
    ]);
    let list = fixtures.check("goals_list_paused", r.request("goals.list", json!({})));
    for id in &goals[..3] {
        let g = named(&list, id);
        assert_eq!(g["paused"], true);
        assert_eq!(g["progress"]["met"], false);
        assert_eq!(g["paused_project"], json!({"id":p,"name":"Study"}));
    }
    assert_eq!(named(&list, &goals[3])["progress"]["met"], true);
    let status = fixtures.check("status_project_paused", r.request("status.get", json!({})));
    assert_eq!(status["goals"].as_array().unwrap().len(), 1);
    drop(r);
    let mut r = on(&h, FRI);
    archive(&mut r, &p, "resume", false);
    let args = json!({"id":goals[0],"from":MON,"to":FRI});
    let history = fixtures.check("goals_history_paused", r.request("goals.history", args));
    let rows = history.as_array().unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r["paused"].as_bool().unwrap())
            .collect::<Vec<_>>(),
        [false, true, true, true, false]
    );
    let scored: Vec<_> = rows.iter().filter(|r| r["paused"] == false).collect();
    assert_eq!(scored.len(), 2);
    assert_eq!(
        scored.iter().rev().take_while(|r| r["met"] == true).count(),
        2
    );
    assert_eq!(
        scored.iter().filter(|r| r["met"] == true).count(),
        scored.len()
    );
    let historic = good(&mut r, "goals.list", json!({"at":WED}));
    assert_eq!(named(&historic, &goals[2])["paused"], true);
    let current = good(&mut r, "goals.list", json!({}));
    assert_eq!(named(&current, &goals[1])["paused"], true);
    assert_eq!(named(&current, &goals[0])["paused"], false);
    let review = good(&mut r, "review.get", json!({"week":MON}));
    assert_eq!(review["summary"]["goals_met_count"], 5);
    assert_eq!(review["summary"]["by_project"][0]["minutes"], 30);
    let all = good(
        &mut r,
        "goals.history",
        json!({"id":goals[3],"from":MON,"to":FRI}),
    );
    assert_eq!(all[0]["paused"], false);
    assert_eq!(all[0]["value"], 30.0);

    good(
        &mut r,
        "item.edit",
        json!({"op_id":"move-away","id":routine,"project_id":null}),
    );
    assert_eq!(
        named(&good(&mut r, "goals.list", json!({"at":WED})), &goals[2])["paused"],
        true
    );
}

#[test]
fn rpc_migrates_archived_projects_on_the_configured_tracking_day_after_sleep() {
    let h = Harness::new();
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../salah-store/src/migrations");
    for file in [
        "001_schema.sql",
        "tests/tracking_v1.sql",
        "002_history.sql",
        "003_tracking_ui.sql",
        "004_epics_backlog_resources.sql",
        "005_goals_reviews_item_tags.sql",
        "006_routine_end_day.sql",
    ] {
        super::sqlite(&h, &std::fs::read_to_string(migrations.join(file)).unwrap());
    }
    super::sqlite(
        &h,
        "UPDATE projects SET archived=1 WHERE id='p'; PRAGMA user_version=6",
    );
    // Use a custom Sleep boundary. The local and UTC calendars are still Wednesday.
    let mut config = salah_core::config::load_from(&h.config).unwrap();
    config.sleep_time = "20:00".into();
    salah_core::config::save_to(&config, &h.config).unwrap();
    let mut r = Session::open(&h, "2031-05-14T20:30:00+05:00");
    assert_eq!(
        super::sqlite(
            &h,
            "SELECT project_id,from_day FROM project_pauses ORDER BY project_id"
        ),
        "a|2031-05-15\np|2031-05-15\n"
    );
    assert!(
        day_items(&good(&mut r, "day.get", json!({"date":WED})))
            .iter()
            .any(|i| i["id"] == "daily")
    );
    assert!(
        !day_items(&good(&mut r, "day.get", json!({"date":THU})))
            .iter()
            .any(|i| i["id"] == "daily")
    );
}
