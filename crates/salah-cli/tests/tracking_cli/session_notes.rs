use super::{Fixture, LATER, stderr};
use serde_json::json;

#[test]
fn timed_note_commands_and_filtered_notes_and_insights_round_trip() {
    let f = Fixture::new();
    let task = f.added("items", &["item", "add", "Read", "--json"]);
    let session = f.added("sessions", &["session", "start", "--item", &task, "--json"]);
    f.json_at(
        LATER,
        &[
            "session",
            "stop",
            "--note",
            "Main",
            "--value",
            "distractions=3",
            "--json",
        ],
    );
    let added = f.json_at(
        LATER,
        &[
            "session",
            "note",
            "First",
            "--id",
            &session,
            "--at",
            "09:40",
            "--expected-version",
            "2",
            "--json",
        ],
    );
    let id = added["changed"]["sessions"][0]["notes"][0]["id"]
        .as_str()
        .unwrap();
    let edited = f.json_at(
        LATER,
        &["session", "note-edit", &session, id, "Revised", "--json"],
    );
    assert_eq!(
        edited["changed"]["sessions"][0]["notes"][0]["text"],
        "Revised"
    );
    let first = f.json_at(
        LATER,
        &[
            "notes",
            "list",
            "--days",
            "1",
            "--item",
            &task[..8],
            "--min",
            "distractions_per_hour=7",
            "--has-value",
            "distractions",
            "--limit",
            "1",
            "--json",
        ],
    );
    assert_eq!(first["total"], 2);
    let second = f.json_at(
        LATER,
        &[
            "notes",
            "list",
            "--days",
            "1",
            "--item",
            &task[..8],
            "--min",
            "distractions_per_hour=7",
            "--has-value",
            "distractions",
            "--limit",
            "1",
            "--before",
            first["next"].as_str().unwrap(),
            "--json",
        ],
    );
    assert_eq!(second["rows"][0]["text"], "Revised");
    assert!(second["next"].is_null());
    let insights = f.json_at(
        LATER,
        &[
            "insights",
            "--days",
            "1",
            "--min",
            "distractions_per_hour=8",
            "--json",
        ],
    );
    assert_eq!(insights["best"], json!([]));
    assert_eq!(insights["by_weekday"][2]["minutes"], 0);
    assert_eq!(insights["on_plan"][0]["minutes"], 25);
    let bad = f.run(&["insights", "--max", "distractions_per_hour=NaN"]);
    assert!(!bad.status.success() && stderr(&bad).contains("finite"));
    let removed = f.json_at(
        LATER,
        &[
            "session",
            "note-rm",
            &session,
            id,
            "--expected-version",
            "4",
            "--json",
        ],
    );
    assert_eq!(removed["changed"]["sessions"][0]["notes"], json!([]));
}
