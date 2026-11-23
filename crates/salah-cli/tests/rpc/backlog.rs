use super::phase2::{added, bad, good};
use super::{DATE, Fixtures, Harness, NOW, Session, id};
use serde_json::{Value, json};

const TOMORROW: &str = "2031-05-15";
const NEXT_DAY: &str = "2031-05-15T10:00:00+05:00";

fn titles(rpc: &mut Session, date: &str) -> Vec<String> {
    let day = good(rpc, "day.get", json!({"date":date}));
    day["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|b| b["items"].as_array().unwrap().clone())
        .map(|row| row["title"].as_str().unwrap().to_owned())
        .collect()
}

fn backlog_titles(rpc: &mut Session, params: Value) -> Vec<String> {
    good(rpc, "backlog.list", params)
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["title"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn backlog_tasks_stay_out_of_every_plan_until_scheduled() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let mut fixtures = Fixtures::default();
    let tag = added(
        &mut rpc,
        "tag.add",
        "tags",
        json!({"op_id":"tag","name":"Study","color":"#223344"}),
    );
    let project = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"project","name":"Book","color":"#335577","tag_ids":[tag]}),
    );
    let read = id(
        &fixtures.check(
            "item_add_backlog",
            rpc.request(
                "item.add",
                json!({"op_id":"read","title":"Read chapter","backlog":true,
                       "project_id":project,"planned_minutes":40}),
            ),
        ),
        "items",
    );
    let replay = good(&mut rpc, "item.add", json!({"op_id":"read"}));
    assert_eq!(replay["changed"]["items"][0]["backlog"], true);
    let call = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"call","quick_text":"call the bank 15m after asr tomorrow","backlog":true}),
    );
    for (params, field) in [
        (
            json!({"op_id":"bad-repeat","title":"Daily","backlog":true,"repeat":"daily"}),
            "repeat",
        ),
        (
            json!({"op_id":"bad-block","title":"Block","backlog":true,"block":"fajr"}),
            "backlog",
        ),
        (
            json!({"op_id":"bad-date","title":"Date","backlog":true,"date":DATE}),
            "backlog",
        ),
    ] {
        bad(&mut rpc, "item.add", params, "validation", Some(field));
    }
    let today = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"today","title":"Pay rent","block":"fajr","planned_minutes":10}),
    );
    let routine = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"routine","title":"Walk","repeat":"daily","block":"asr"}),
    );

    // Backlog items are in no plan, capacity or fulfilment.
    assert_eq!(titles(&mut rpc, DATE), ["Pay rent", "Walk"]);
    assert!(!titles(&mut rpc, TOMORROW).contains(&"Call the bank".to_owned()));
    let stats = good(&mut rpc, "stats.get", json!({"days":1}));
    assert_eq!(stats["days"][0]["planned_minutes"], 10);
    let call_row = good(&mut rpc, "backlog.list", json!({}))[1].clone();
    assert_eq!(
        (
            &call_row["title"],
            &call_row["planned_minutes"],
            &call_row["block"]
        ),
        (&json!("Call the bank"), &json!(15), &Value::Null)
    );

    // Order within the backlog; a move never crosses into a day.
    good(
        &mut rpc,
        "item.move",
        json!({"op_id":"reorder","id":call,"block":null,"index":0}),
    );
    assert_eq!(
        backlog_titles(&mut rpc, json!({})),
        ["Call the bank", "Read chapter"]
    );
    bad(
        &mut rpc,
        "item.move",
        json!({"op_id":"cross","id":call,"block":"fajr"}),
        "validation",
        Some("block"),
    );
    bad(
        &mut rpc,
        "item.move",
        json!({"op_id":"out","id":today,"block":null}),
        "validation",
        Some("block"),
    );
    bad(
        &mut rpc,
        "item.move",
        json!({"op_id":"missing","id":call}),
        "validation",
        None,
    );
    bad(
        &mut rpc,
        "item.edit",
        json!({"op_id":"edit-block","id":call,"block":"isha"}),
        "validation",
        Some("block"),
    );
    bad(
        &mut rpc,
        "item.edit",
        json!({"op_id":"edit-repeat","id":call,"repeat":"daily"}),
        "validation",
        Some("repeat"),
    );
    let renamed = good(
        &mut rpc,
        "item.edit",
        json!({"op_id":"rename","id":call,"title":"Call the bank about fees"}),
    );
    assert_eq!(renamed["changed"]["items"][0]["backlog"], true);

    // Filters: project, tag and the unassigned rows.
    fixtures.check(
        "backlog_list",
        rpc.request("backlog.list", json!({"tag_id":tag})),
    );
    assert_eq!(
        backlog_titles(&mut rpc, json!({"project_id":"none"})),
        ["Call the bank about fees"]
    );
    assert_eq!(
        backlog_titles(&mut rpc, json!({"tag_id":"none"})),
        ["Call the bank about fees"]
    );
    assert_eq!(
        backlog_titles(&mut rpc, json!({"project_id":project})),
        ["Read chapter"]
    );
    bad(
        &mut rpc,
        "backlog.list",
        json!({"project_id":"missing"}),
        "not_found",
        None,
    );

    // Scheduling gives it a day and block; the backlog forgets it.
    let scheduled = fixtures.check(
        "item_schedule",
        rpc.request(
            "item.schedule",
            json!({"op_id":"schedule","id":read,"day":TOMORROW,"block":"isha","expected_version":2}),
        ),
    );
    assert_eq!(scheduled["changed"]["items"][0]["backlog"], false);
    assert_eq!(
        good(&mut rpc, "item.schedule", json!({"op_id":"schedule"})),
        scheduled
    );
    assert!(titles(&mut rpc, TOMORROW).contains(&"Read chapter".to_owned()));
    assert_eq!(
        backlog_titles(&mut rpc, json!({})),
        ["Call the bank about fees"]
    );
    bad(
        &mut rpc,
        "item.schedule",
        json!({"op_id":"sleep","id":call,"day":DATE,"block":"sleep"}),
        "validation",
        None,
    );
    for (params, field) in [
        (
            json!({"op_id":"past","id":call,"day":"2031-05-13","block":"fajr"}),
            "day",
        ),
        (
            json!({"op_id":"routine-schedule","id":routine,"day":DATE,"block":"fajr"}),
            "repeat",
        ),
    ] {
        bad(&mut rpc, "item.schedule", params, "validation", Some(field));
    }
    bad(
        &mut rpc,
        "item.backlog",
        json!({"op_id":"routine-backlog","id":routine}),
        "validation",
        Some("repeat"),
    );

    // Work on a backlog task is unplanned work.
    let logged = good(
        &mut rpc,
        "session.log",
        json!({"op_id":"log","item_id":call,"minutes":5}),
    );
    assert_eq!(logged["changed"]["sessions"][0]["item_id"], call);
    let stats = good(&mut rpc, "stats.get", json!({"days":1}));
    assert_eq!(stats["days"][0]["planned_minutes"], 10);
    assert_eq!(stats["days"][0]["unplanned_minutes"], 5);
    drop(rpc);

    // Back to the backlog the next day: the frozen past day keeps the task.
    let mut next = Session::open(&h, NEXT_DAY);
    let moved = fixtures.check(
        "item_backlog",
        next.request("item.backlog", json!({"op_id":"shelve","id":today})),
    );
    assert_eq!(moved["changed"]["items"][0]["backlog"], true);
    assert!(moved["changed"]["items"][0]["day"].is_null());
    assert_eq!(titles(&mut next, DATE), ["Pay rent", "Walk"]);
    let past = good(&mut next, "stats.get", json!({"from":DATE,"to":DATE}));
    assert_eq!(past["days"][0]["planned_minutes"], 10);
    let again = good(
        &mut next,
        "item.backlog",
        json!({"op_id":"shelve-again","id":today}),
    );
    assert_eq!(
        again["changed"]["items"][0]["version"],
        moved["changed"]["items"][0]["version"]
    );
    assert_eq!(
        backlog_titles(&mut next, json!({})),
        ["Call the bank about fees", "Pay rent"]
    );
    // Today's plan only has what is still scheduled for it.
    assert_eq!(titles(&mut next, TOMORROW), ["Walk", "Read chapter"]);
}
