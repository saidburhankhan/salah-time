use super::{
    DATE, Fixtures, Harness, Session,
    phase2::{added, bad, good},
};
use serde_json::{Value, json};

#[test]
fn insights_allocate_time_group_dates_and_resolve_filters_and_notes() {
    let h = Harness::new();
    // Equal real spans on either side of the harness's 12:19 Dhuhr boundary.
    let mut rpc = Session::open(&h, "2031-05-14T11:49:00+05:00");
    let tag = added(
        &mut rpc,
        "tag.add",
        "tags",
        json!({"op_id":"tag","name":"Books","color":"#445566"}),
    );
    let p = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"project","name":"Study","tag_ids":[tag]}),
    );
    let e = added(
        &mut rpc,
        "epic.add",
        "epics",
        json!({"op_id":"epic","name":"Chapter","project_id":p}),
    );
    let task = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"task","title":"Read","epic_id":e,"block":"fajr"}),
    );
    let loose = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"free","title":"Walk","backlog":true}),
    );
    good(
        &mut rpc,
        "session.start",
        json!({"op_id":"timer","item_id":task}),
    );
    good(
        &mut rpc,
        "session.note",
        json!({"op_id":"entry","text":"Paragraph 2"}),
    );
    drop(rpc);
    let mut rpc = Session::open(&h, "2031-05-14T12:49:00+05:00");
    good(
        &mut rpc,
        "session.stop",
        json!({"op_id":"stop","minutes":30,"note":"Chapter 1","values":{"focus":5,"distractions":2}}),
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"log","item_id":loose,"minutes":10,"values":{"focus":1}}),
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"tick","item_id":task,"values":{"focus":3,"distractions":20}}),
    );
    let report = Fixtures::default().check(
        "insights",
        rpc.request("insights.get", json!({"from":"2031-05-11","to":DATE})),
    );
    assert_eq!(report["by_block"][1]["minutes"], 15);
    assert_eq!(report["by_block"][2]["minutes"], 25);
    assert_eq!(report["by_block"][2]["sessions"], 2);
    assert_eq!(report["by_block"][2]["focus_avg"], 3.4);
    assert_eq!(report["by_block"][2]["distractions_per_hour"], 2.4);
    assert_eq!(report["by_hour"][11]["minutes"], 30);
    assert_eq!(report["by_hour"][12]["sessions"], 0);
    assert_eq!(report["by_weekday"][2]["weekday"], 3);
    assert_eq!(report["by_weekday"][2]["focus_avg"], 4.);
    assert_eq!(
        report["distractions_by_week"],
        json!([{"week":"2031-05-05","per_hour":null},{"week":"2031-05-12","per_hour":3.}])
    );
    assert_eq!(report["best"][0]["item"]["title"], "Read");
    assert_eq!(report["best"][0]["project"]["name"], "Study");
    assert_eq!(report["best"][0]["notes"][0]["text"], "Paragraph 2");
    assert_eq!(report["hardest"][0]["item_id"], loose);
    let plan = report["on_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["item_id"] == task)
        .unwrap();
    assert_eq!(plan["on_plan_minutes"], 15);
    assert_eq!(plan["minutes"], 30);
    assert_eq!(
        report["block_matrix"][0],
        json!({"planned_block":null,"actual_block":"dhuhr","minutes":10})
    );
    for (key, id, minutes) in [
        ("item_id", task.as_str(), 30),
        ("tag_id", tag.as_str(), 30),
        ("project_id", p.as_str(), 30),
        ("epic_id", e.as_str(), 30),
        ("project_id", "none", 10),
        ("epic_id", "none", 10),
        ("tag_id", "none", 10),
        ("item_id", "none", 0),
    ] {
        let r = good(
            &mut rpc,
            "insights.get",
            json!({"from":DATE,"to":DATE,key:id}),
        );
        assert_eq!(r["by_weekday"][2]["minutes"], minutes);
    }
    let empty = good(
        &mut rpc,
        "insights.get",
        json!({"from":"2031-05-01","to":"2031-05-01"}),
    );
    assert_eq!(empty["by_weekday"][3]["focus_avg"], Value::Null);
    assert_eq!(empty["best"], json!([]));
    assert_eq!(empty["block_matrix"], json!([]));
    bad(
        &mut rpc,
        "insights.get",
        json!({"from":"2028-05-01","to":DATE}),
        "validation",
        Some("from"),
    );
}

#[test]
fn insights_rank_only_five_scored_sessions_breaking_ties_by_minutes() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, super::NOW);
    let p = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"p","name":"Study"}),
    );
    for (i, f) in [1, 1, 2, 3, 4, 5, 5].into_iter().enumerate() {
        good(
            &mut rpc,
            "session.log",
            json!({"op_id":format!("log{i}"),"project_id":p,"minutes":i+1,"values":{"focus":f}}),
        );
    }
    let result = good(&mut rpc, "insights.get", json!({"days":1}));
    for (field, expected) in [("best", [7, 6, 5, 4, 3]), ("hardest", [2, 1, 3, 4, 5])] {
        let minutes: Vec<_> = result[field]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["minutes"].as_i64().unwrap())
            .collect();
        assert_eq!(minutes, expected);
    }
}
