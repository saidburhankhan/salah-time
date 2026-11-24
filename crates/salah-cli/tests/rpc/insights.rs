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

#[test]
fn itemless_project_and_epic_work_share_an_unplanned_row_after_filtering() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, super::NOW);
    let p = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"p","name":"Study"}),
    );
    let e = added(
        &mut rpc,
        "epic.add",
        "epics",
        json!({"op_id":"e","project_id":p,"name":"Book"}),
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"project","project_id":p,"minutes":45}),
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"epic","epic_id":e,"minutes":15,"block":"isha"}),
    );
    for (filter, minutes) in [
        (json!({"project_id":p}), 60),
        (json!({"epic_id":e}), 15),
        (json!({"project_id":p,"epic_id":"none"}), 45),
    ] {
        let report = good(&mut rpc, "insights.get", filter);
        assert_eq!(
            report["on_plan"],
            json!([{"item_id":null,"name":"Unplanned","planned_block":null,"minutes":minutes,"on_plan_minutes":0}])
        );
        let cells = report["block_matrix"].as_array().unwrap();
        assert!(cells.iter().all(|c| c["planned_block"].is_null()));
        assert_eq!(
            cells
                .iter()
                .map(|c| c["minutes"].as_i64().unwrap())
                .sum::<i64>(),
            minutes
        );
    }
}

#[test]
fn insights_uses_one_task_rate_across_a_historical_block_move() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, super::NOW);
    let item = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"routine","title":"Read","block":"fajr","repeat":"daily"}),
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"first","item_id":item,"block":"fajr","minutes":30}),
    );
    drop(rpc);
    let mut rpc = Session::open(&h, "2031-05-15T10:00:00+05:00");
    good(
        &mut rpc,
        "item.edit",
        json!({"op_id":"move","id":item,"block":"asr"}),
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"second","item_id":item,"block":"fajr","minutes":30}),
    );
    let result = good(&mut rpc, "insights.get", json!({"days":2,"item_id":item}));
    assert_eq!(
        result["on_plan"],
        json!([{"item_id":item,"name":"Read","planned_block":null,
        "planned_blocks":["fajr","asr"],"on_plan_minutes":30,"minutes":60}])
    );
    assert_eq!(
        result["block_matrix"],
        json!([
            {"planned_block":"fajr","actual_block":"fajr","minutes":30},
            {"planned_block":"asr","actual_block":"fajr","minutes":30}
        ])
    );
}

#[test]
fn ranked_notes_fit_the_frame_and_every_entry_can_be_opened() {
    use salah_domain::tracking::SessionNote;
    let h = Harness::new();
    let mut rpc = Session::open(&h, super::NOW);
    let item = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"op_id":"read","title":"Read"}),
    );
    let id = added(
        &mut rpc,
        "session.log",
        "sessions",
        json!({"op_id":"log","item_id":item,"minutes":30,"note":"Main note","values":{"focus":5}}),
    );
    let body = "📖".repeat(500);
    let mut store = salah_store::Store::open(&h.data).unwrap();
    store
        .write(|tx| {
            for i in 0..300 {
                tx.add_session_note(
                    &id,
                    &SessionNote {
                        id: format!("note-{i}"),
                        at: super::NOW.parse().unwrap(),
                        text: body.clone(),
                    },
                )?;
            }
            Ok(())
        })
        .unwrap();
    let response = rpc.request("insights.get", json!({"days":1}));
    assert!(serde_json::to_vec(&response).unwrap().len() + 1 < salah_protocol::MAX_LINE_BYTES);
    for key in ["best", "hardest"] {
        let row = &response["result"][key][0];
        assert_eq!(row["id"], id);
        assert_eq!(row["note"], "Main note");
        assert_eq!(row["notes_count"], 300);
        assert_eq!(row["notes_more"], true);
        assert_eq!(row["notes"].as_array().unwrap().len(), 12);
        assert_eq!(row["notes"][0]["id"], "note-288");
    }
    let mut after = Value::Null;
    let mut notes = Vec::new();
    for page in 0..3 {
        let response = rpc.request("session.notes", json!({"id":id,"limit":100,"after":after}));
        assert!(serde_json::to_vec(&response).unwrap().len() + 1 < salah_protocol::MAX_LINE_BYTES);
        let result = &response["result"];
        notes.extend_from_slice(result["notes"].as_array().unwrap());
        after = result["next"].clone();
        assert_eq!(after.is_null(), page == 2);
    }
    assert_eq!(notes.len(), 300);
    for (index, entry) in notes.iter().enumerate() {
        assert_eq!(entry["id"], format!("note-{index}"));
        assert_eq!(entry["text"], body);
    }
}
