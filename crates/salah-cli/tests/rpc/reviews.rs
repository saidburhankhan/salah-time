use super::{
    Fixtures, Harness, NOW, Session,
    phase2::{added, bad, good},
};
use serde_json::{Value, json};

const WEEK: &str = "2031-05-12";

#[test]
fn review_summarizes_both_goal_periods_notes_unassigned_work_and_prior_rate() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, "2031-05-12T09:00:00+05:00");
    let project = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"name":"Study","op_id":"p"}),
    );
    let epic = added(
        &mut rpc,
        "epic.add",
        "epics",
        json!({"name":"Chapter","project_id":project,"op_id":"e"}),
    );
    let task = added(
        &mut rpc,
        "item.add",
        "items",
        json!({"title":"Errand","backlog":true,"op_id":"t"}),
    );
    good(
        &mut rpc,
        "goal.add",
        json!({"period":"day","target":10,"op_id":"daily"}),
    );
    good(
        &mut rpc,
        "goal.add",
        json!({"period":"week","target":30,"op_id":"weekly"}),
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"prev","date":"2031-05-11","block":"isha","item_id":task,"minutes":30,"values":{"distractions":3}}),
    );
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"mon","epic_id":epic,"minutes":10,"note":"Monday","values":{"distractions":1}}),
    );
    drop(rpc);
    let mut rpc = Session::open(&h, NOW);
    good(
        &mut rpc,
        "session.start",
        json!({"op_id":"start","item_id":task}),
    );
    good(
        &mut rpc,
        "session.note",
        json!({"op_id":"entry","text":"At the shop"}),
    );
    drop(rpc);
    let mut rpc = Session::open(&h, super::LATER);
    good(
        &mut rpc,
        "session.stop",
        json!({"op_id":"stop","minutes":20}),
    );
    let open = good(&mut rpc, "review.get", json!({"week":WEEK}));
    assert_eq!(open["summary"]["open_backlog_count"], 1);
    good(
        &mut rpc,
        "session.log",
        json!({"op_id":"tick","item_id":task,"values":{"distractions":10}}),
    );
    let save = json!({"op_id":"save","week":WEEK,"note":"Keep reading.\nTry mornings."});
    let mut fixtures = Fixtures::default();
    let saved = fixtures.check("review_save", rpc.request("review.save", save.clone()));
    assert_eq!(good(&mut rpc, "review.save", save), saved);
    let review = fixtures.check(
        "review_get",
        rpc.request("review.get", json!({"week":WEEK})),
    );
    assert_eq!(review["note"], "Keep reading.\nTry mornings.");
    let s = &review["summary"];
    assert_eq!(s["goals_met"].as_array().unwrap().len(), 3);
    assert_eq!(s["goals_met"][1]["progress"]["from"], super::DATE);
    assert_eq!(s["goals_met"][2]["progress"]["value"], 30.);
    assert_eq!(
        s["by_project"],
        json!([{"id":"none","name":"No project","minutes":20},{"id":project,"name":"Study","minutes":10}])
    );
    assert_eq!(
        s["by_epic"],
        json!([{"id":epic,"name":"Chapter","minutes":10},{"id":"none","name":"No epic","minutes":20}])
    );
    assert_eq!(s["sessions_with_notes"].as_array().unwrap().len(), 2);
    assert_eq!(
        s["sessions_with_notes"][0]["notes"][0]["text"],
        "At the shop"
    );
    assert_eq!(s["sessions_with_notes"][1]["note"], "Monday");
    assert_eq!(s["distractions_per_hour"], 22.);
    let table = good(
        &mut rpc,
        "stats.table",
        json!({"from":"2031-05-05","to":"2031-05-18","group":"week"}),
    );
    for (row, key) in [
        (0, "previous_distractions_per_hour"),
        (1, "distractions_per_hour"),
    ] {
        assert_eq!(
            s[key],
            table["rows"][row]["values"]["distractions_per_hour"]
        );
    }
    let insights = good(
        &mut rpc,
        "insights.get",
        json!({"from":WEEK,"to":"2031-05-18"}),
    );
    assert_eq!(insights["distractions_by_week"][0]["per_hour"], 2.);
    assert_eq!(s["previous_distractions_per_hour"], 6.);
    let following = good(&mut rpc, "review.get", json!({"week":"2031-05-19"}));
    assert_eq!(following["summary"]["previous_distractions_per_hour"], 22.);
    assert!(following["summary"]["distractions_per_hour"].is_null());
    assert_eq!(s["open_backlog_count"], 0);
    let updated = good(
        &mut rpc,
        "review.save",
        json!({"week":WEEK,"op_id":"edit","note":"Revised"}),
    );
    let row = &updated["changed"]["reviews"][0];
    assert_eq!(row["version"], 2);
    assert_eq!(saved["changed"]["reviews"][0]["id"], row["id"]);
    let clear = json!({"week":WEEK,"op_id":"clear","note":""});
    let deleted = good(&mut rpc, "review.save", clear.clone());
    assert_eq!(deleted["changed"]["deleted"], json!([row["id"]]));
    assert_eq!(good(&mut rpc, "review.save", clear), deleted);
    let cleared = good(&mut rpc, "review.get", json!({"week":WEEK}));
    assert!(cleared["note"].is_null() && cleared["updated_at"].is_null());
    assert_eq!(cleared["summary"], *s);
    let recreated = good(
        &mut rpc,
        "review.save",
        json!({"week":WEEK,"op_id":"again","note":"Again"}),
    );
    assert_ne!(recreated["changed"]["reviews"][0]["id"], row["id"]);
    let store = salah_store::Store::open(&h.data).unwrap();
    let changes = store.changes_since(0, 100).unwrap();
    let reviews: Vec<_> = changes
        .iter()
        .filter(|c| c.entity == "reviews")
        .map(|c| (c.op.as_str(), c.version))
        .collect();
    assert_eq!(
        reviews,
        [("insert", 1), ("update", 2), ("delete", 3), ("insert", 1)]
    );
}

#[test]
fn reviews_validate_monday_and_limit_unicode_reflections() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    for date in ["2031-5-12", "2031-05-13", "2031-02-30", "yesterday"] {
        bad(
            &mut rpc,
            "review.get",
            json!({"week":date}),
            "validation",
            Some("week"),
        );
        bad(
            &mut rpc,
            "review.save",
            json!({"week":date,"note":"Text","op_id":"wrong"}),
            "validation",
            Some("week"),
        );
    }
    let empty = good(&mut rpc, "review.get", json!({"week":WEEK}));
    assert_eq!(empty["summary"]["distractions_per_hour"], Value::Null);
    assert_eq!(
        empty["summary"]["previous_distractions_per_hour"],
        Value::Null
    );
    assert_eq!(empty["summary"]["goals_met"], json!([]));
    for invalid in ["é".repeat(5001), "embedded\0NUL".into()] {
        bad(
            &mut rpc,
            "review.save",
            json!({"week":WEEK,"op_id":"invalid","note":invalid}),
            "validation",
            Some("note"),
        );
    }
    good(
        &mut rpc,
        "review.save",
        json!({"week":WEEK,"op_id":"maximum","note":"é".repeat(5000)}),
    );
}

#[test]
fn a_large_week_keeps_its_summary_and_reflection_with_bounded_note_previews() {
    use salah_domain::tracking::SessionNote;
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let p = added(
        &mut rpc,
        "project.add",
        "projects",
        json!({"op_id":"p","name":"Work"}),
    );
    let note = "📖".repeat(500);
    let reflection = "🌿".repeat(5000);
    good(
        &mut rpc,
        "review.save",
        json!({"op_id":"save","week":WEEK,"note":reflection}),
    );
    let mut store = salah_store::Store::open(&h.data).unwrap();
    for i in 0..40 {
        let id = added(
            &mut rpc,
            "session.log",
            "sessions",
            json!({"op_id":format!("s{i}"),"project_id":p,"minutes":10}),
        );
        store
            .write(|tx| {
                for j in 0..if i == 0 { 600 } else { 12 } {
                    tx.add_session_note(
                        &id,
                        &SessionNote {
                            id: format!("note-{i}-{j}"),
                            at: NOW.parse().unwrap(),
                            text: note.clone(),
                        },
                    )?;
                }
                Ok(())
            })
            .unwrap();
        if i == 0 {
            let response = rpc.request("review.get", json!({"week":WEEK}));
            assert!(
                serde_json::to_vec(&response).unwrap().len() + 1 < salah_protocol::MAX_LINE_BYTES
            );
            let s = &response["result"]["summary"];
            assert_eq!(s["sessions_with_notes_count"], 1);
            assert_eq!(s["sessions_with_notes_more"], false);
            let row = &s["sessions_with_notes"][0];
            assert_eq!(row["notes_count"], 600);
            assert_eq!(row["notes_more"], true);
            assert_eq!(row["notes"][0]["id"], "note-0-588");
            assert_eq!(row["notes"][11]["id"], "note-0-599");
        }
    }
    let response = rpc.request("review.get", json!({"week":WEEK}));
    assert!(serde_json::to_vec(&response).unwrap().len() + 1 < salah_protocol::MAX_LINE_BYTES);
    assert_eq!(response["result"]["note"], reflection);
    let s = &response["result"]["summary"];
    assert_eq!(s["by_project"][0]["minutes"], 400);
    assert_eq!(s["sessions_with_notes_count"], 40);
    assert_eq!(s["sessions_with_notes_more"], true);
    let shown = s["sessions_with_notes"].as_array().unwrap().len();
    assert!(
        (1..32).contains(&shown),
        "the byte budget should bound this preview: {shown}"
    );
}
