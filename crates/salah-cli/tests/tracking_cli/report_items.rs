use super::Fixture;

#[test]
fn report_item_filters_resolve_removed_ids_and_prefixes_while_edits_require_live_items() {
    let f = Fixture::new();
    let task = f.added("items", &["item", "add", "Reading", "--json"]);
    f.json(&[
        "session",
        "log",
        "--item",
        &task,
        "--minutes",
        "12",
        "--note",
        "Finished",
        "--json",
    ]);
    f.json(&["item", "remove", &task, "--json"]);
    for id in [task.as_str(), &task[..8]] {
        let insights = f.json(&["insights", "--days", "1", "--item", id, "--json"]);
        assert_eq!(insights["on_plan"][0]["item_id"], task);
        assert_eq!(insights["on_plan"][0]["minutes"], 12);
        let stats = f.json(&["stats", "--days", "1", "--item", id, "--json"]);
        assert_eq!(stats["totals"]["minutes"], 12);
        let series = f.json(&[
            "stats",
            "series",
            "--days",
            "1",
            "--item",
            id,
            "--measure",
            "minutes",
        ]);
        assert_eq!(series["series"][0]["points"][0]["value"], 12.);
        let table = f.json(&["stats", "table", "--days", "1", "--item", id]);
        assert_eq!(table["totals"]["minutes"], 12);
        let notes = f.json(&["sessions", "list", "--days", "1", "--item", id]);
        assert_eq!(notes["rows"][0]["note"], "Finished");
        let edit = f.run(&["item", "edit", id, "--title", "Renamed"]);
        assert!(!edit.status.success());
        assert!(super::stderr(&edit).contains("no item"));
    }
}
