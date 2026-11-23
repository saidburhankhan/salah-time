//! The human CLI end to end: every tracking subcommand runs the real binary
//! against a temporary store and goes through the protocol dispatcher.

use chrono::{DateTime, Days};
use salah_core::config::Config;
use salah_store::Store;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicUsize = AtomicUsize::new(0);
/// Inside the Fajr block of 2031-05-14 in the default location.
const NOW: &str = "2031-05-14T09:30:00+05:00";
const LATER: &str = "2031-05-14T09:55:00+05:00";

struct Fixture {
    root: PathBuf,
    config: PathBuf,
    data: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "salah-tracking-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let config = root.join("config.json");
        salah_core::config::save_to(
            &Config {
                method: "mwl".into(),
                ..Config::default()
            },
            &config,
        )
        .unwrap();
        let data = root.join("salah.db");
        Self { root, config, data }
    }

    fn run_at(&self, now: &str, args: &[&str]) -> Output {
        run(&self.config, &self.data, now, args)
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_at(NOW, args)
    }

    fn json_at(&self, now: &str, args: &[&str]) -> Value {
        let output = self.run_at(now, args);
        assert!(output.status.success(), "{args:?}: {}", stderr(&output));
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn json(&self, args: &[&str]) -> Value {
        self.json_at(NOW, args)
    }

    /// Runs a mutation and returns the id of the first changed `kind` row.
    fn added(&self, kind: &str, args: &[&str]) -> String {
        self.json(args)["changed"][kind][0]["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn run(config: &Path, data: &Path, now: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_salah"))
        .env("SALAH_CONFIG", config)
        .env("SALAH_DATA", data)
        .env("SALAH_NOW", now)
        .args(args)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn resource_commands_list_add_edit_and_remove() {
    let fixture = Fixture::new();
    let task = fixture.added("items", &["backlog", "add", "Read", "--json"]);
    let reminder = fixture.added(
        "resources",
        &[
            "resource",
            "add",
            "--item",
            &task[..8],
            "--kind",
            "reminder",
            "--remind-at",
            "2031-05-14T04:00:00Z",
            "--title",
            "Read now",
            "--json",
        ],
    );
    let listed = fixture.run(&["resource", "list", "--item", &task]);
    assert!(listed.status.success(), "{}", stderr(&listed));
    let list = String::from_utf8_lossy(&listed.stdout);
    assert!(list.contains(&reminder) && list.contains("Read now"));

    let done = fixture.json(&["resource", "edit", &reminder, "--done", "--json"]);
    assert_eq!(done["changed"]["resources"][0]["done"], true);
    let reopened = fixture.json(&["resource", "edit", &reminder, "--undone", "--json"]);
    assert_eq!(reopened["changed"]["resources"][0]["done"], false);
    let removed = fixture.json(&["resource", "remove", &reminder, "--json"]);
    assert_eq!(removed["changed"]["deleted"][0], reminder);
    let list = fixture.json(&["resource", "list", "--item", &task, "--json"]);
    assert_eq!(list, serde_json::json!([]));

    let project = fixture.added("projects", &["project", "add", "Reading", "--json"]);
    let epic = fixture.added(
        "epics",
        &["epic", "add", "Part 1", "--project", &project, "--json"],
    );
    fixture.json(&[
        "resource", "add", "--epic", &epic, "--kind", "note", "--body", "Outline", "--json",
    ]);
    assert_eq!(
        fixture.json(&["resource", "list", "--epic", &epic, "--json"])[0]["body"],
        "Outline"
    );
    assert!(
        !fixture
            .run(&["resource", "list", "--item", &task, "--epic", &epic])
            .status
            .success()
    );
}

fn day_rows(day: &Value) -> Vec<Value> {
    day["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|block| block["items"].as_array().unwrap().clone())
        .collect()
}

#[test]
fn items_are_planned_edited_moved_and_removed() {
    let fixture = Fixture::new();
    let id = fixture.added(
        "items",
        &[
            "item",
            "add",
            "--quick",
            "--json",
            "--",
            "read 20m after fajr daily",
        ],
    );
    let row = &fixture.json(&["item", "list", "--json"])[0];
    assert_eq!(
        (row["title"].clone(), row["block"].clone()),
        ("Read".into(), "fajr".into())
    );
    assert_eq!(row["planned_minutes"], 20);

    let prefix = &id[..8];
    let edited = fixture.json(&[
        "item",
        "edit",
        prefix,
        "--title=Recite",
        "--no-planned-minutes",
        "--json",
    ]);
    assert_eq!(edited["changed"]["items"][0]["title"], "Recite");
    assert!(edited["changed"]["items"][0]["planned_minutes"].is_null());

    fixture.json(&["item", "move", prefix, "--block", "asr", "--json"]);
    let day = fixture.json(&["day", "--json"]);
    let rows = day_rows(&day);
    assert_eq!((rows.len(), rows[0]["block"].clone()), (1, "asr".into()));

    let removed = fixture.json(&["item", "remove", prefix, "--json"]);
    assert_eq!(removed["changed"]["deleted"][0], id.as_str());
    assert!(
        fixture
            .json(&["item", "list", "--json"])
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn item_edit_catches_up_a_400_day_gap_in_one_bounded_write() {
    let fixture = Fixture::new();
    let daily_item = |title| {
        fixture.added(
            "items",
            &[
                "item",
                "add",
                title,
                "--repeat",
                "daily",
                "--planned-minutes",
                "10",
                "--json",
            ],
        )
    };
    let id = daily_item("Read");
    for _ in 1..24 {
        daily_item("Practice");
    }

    let first = DateTime::parse_from_rfc3339(NOW).unwrap();
    let later = first + Days::new(400);
    let today = later.date_naive();
    let store = Store::open(&fixture.data).unwrap();
    let revision = store.revision().unwrap();
    assert!(
        store
            .written_days(first.date_naive(), today)
            .unwrap()
            .is_empty()
    );

    let start = Instant::now();
    fixture.json_at(
        &later.to_rfc3339(),
        &["item", "edit", &id, "--planned-minutes", "45", "--json"],
    );
    let elapsed = start.elapsed();
    eprintln!("400-day catch-up with 24 daily items took {elapsed:?}");
    assert!(
        elapsed < Duration::from_secs(1),
        "catch-up took {elapsed:?}"
    );

    let written = store
        .written_days(first.date_naive(), today + Days::new(1))
        .unwrap();
    let oldest = today - Days::new(366);
    let yesterday = today - Days::new(1);
    assert_eq!(written.len(), 366);
    assert_eq!(written.first(), Some(&oldest));
    assert_eq!(written.last(), Some(&yesterday));
    for day in [oldest, yesterday] {
        let plan = store.day_plan(day).unwrap();
        assert_eq!(plan.len(), 24);
        assert!(plan.iter().all(|line| line.planned_minutes == Some(10)));
    }
    assert_eq!(store.item(&id).unwrap().unwrap().planned_minutes, Some(45));
    assert_eq!(store.revision().unwrap(), revision + 1);

    // A failed edit rolls back the snapshots it would have frozen.
    let next = later + Days::new(5);
    let failed = fixture.run_at(
        &next.to_rfc3339(),
        &["item", "edit", &id, "--planned-minutes", "0", "--json"],
    );
    assert!(!failed.status.success());
    assert!(stderr(&failed).contains("planned_minutes"));
    assert!(
        store
            .written_days(today, next.date_naive())
            .unwrap()
            .is_empty()
    );
    assert!(store.day_blocks(today).unwrap().is_empty());
    assert!(store.day_plan(today).unwrap().is_empty());
    assert!(store.day_clock(today).unwrap().is_none());
    assert_eq!(store.revision().unwrap(), revision + 1);
}

#[test]
fn pausing_records_time_and_ticking_marks_the_item_done() {
    let fixture = Fixture::new();
    let project = fixture.added(
        "projects",
        &["project", "add", "Quran", "--color", "#00aa00", "--json"],
    );
    let item = fixture.added(
        "items",
        &[
            "item",
            "add",
            "Read",
            "--project",
            &project,
            "--planned-minutes",
            "30",
            "--block",
            "fajr",
            "--json",
        ],
    );
    fixture.json(&["session", "start", "--item", &item[..8], "--json"]);
    let status = fixture.json(&["status", "--json"]);
    assert_eq!(status["running"]["item_id"], item.as_str());
    assert_eq!(status["prompt"].as_array().unwrap().len(), 3);
    assert_eq!(status["prompt"][2]["metric_id"], "distractions");

    let stopped = fixture.json_at(LATER, &["session", "stop", "--json"]);
    assert_eq!(stopped["changed"]["sessions"][0]["minutes"], 25);

    let paused = fixture.json_at(LATER, &["day", "--json"]);
    assert_eq!(day_rows(&paused)[0]["done"], false);

    let tick = fixture.json(&[
        "session", "log", "--item", &item, "--value", "focus=3", "--json",
    ]);
    assert!(tick["changed"]["sessions"][0]["minutes"].is_null());

    let day = fixture.json(&["day", "--json"]);
    let row = &day_rows(&day)[0];
    assert_eq!(
        (row["done"].clone(), row["actual_minutes"].clone()),
        (true.into(), 25.into())
    );
    assert_eq!(row["project"]["name"], "Quran");
    assert_eq!(day["fulfilment"]["done_items"], 1);

    let stats = fixture.json(&["stats", "--days", "7", "--project", &project, "--json"]);
    assert_eq!(
        (
            stats["totals"]["minutes"].clone(),
            stats["totals"]["sessions"].clone()
        ),
        (25.into(), 2.into())
    );
}

#[test]
fn invalid_inputs_change_nothing() {
    let fixture = Fixture::new();
    for args in [
        vec!["item", "add", "Nap", "--block", "sleep"],
        vec!["item", "add", " ", "--json"],
        vec!["item", "remove", "not-an-id"],
        vec!["session", "start"],
        vec!["tag", "add", "Deen", "--color", "green"],
        vec!["metric", "add", "Mood", "--kind", "scale"],
    ] {
        let output = fixture.run(&args);
        assert!(!output.status.success(), "unexpected success: {args:?}");
    }
    let unknown = fixture.run(&["item", "remove", "not-an-id"]);
    assert!(stderr(&unknown).contains("no item"));
    assert!(
        fixture
            .json(&["item", "list", "--json"])
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .json(&["tag", "list", "--json"])
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn status_attaches_the_planner_and_survives_a_store_failure() {
    let fixture = Fixture::new();
    let status = fixture.json(&["status", "--json"]);
    assert!(status["planner"]["open"].is_number());
    assert!(status["running"].is_null());

    let regular = fixture.root.join("regular-file");
    std::fs::write(&regular, b"x").unwrap();
    let output = run(
        &fixture.config,
        &regular.join("salah.db"),
        NOW,
        &["status", "--json"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let status: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(status["planner"].is_null());
    assert!(status["planner_error"].is_string());
}

#[test]
fn save_done_and_item_filtered_stats_are_available_without_a_daemon() {
    let fixture = Fixture::new();
    let item = fixture.added("items", &["item", "add", "Read", "--json"]);
    fixture.json(&["session", "start", "--item", &item, "--json"]);
    let saved = fixture.json_at(
        LATER,
        &["session", "stop", "--minutes", "20", "--done", "--json"],
    );
    let sessions = saved["changed"]["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0]["minutes"], 20);
    assert!(sessions[1]["minutes"].is_null());

    let stats = fixture.json_at(
        LATER,
        &["stats", "--days", "1", "--item", &item[..8], "--json"],
    );
    assert_eq!(stats["filter"]["item_id"], item);
    assert_eq!(stats["totals"]["minutes"], 20);
    assert_eq!(stats["totals"]["sessions"], 2);
}

#[test]
fn stats_table_and_sessions_list_print_the_rpc_json() {
    let fixture = Fixture::new();
    let item = fixture.added("items", &["item", "add", "Read", "--json"]);
    for (minutes, focus, note) in [("30", "focus=4", "Āyat review"), ("10", "focus=1", "")] {
        let mut args = vec![
            "session",
            "log",
            "--item",
            &item,
            "--minutes",
            minutes,
            "--value",
            focus,
            "--json",
        ];
        if !note.is_empty() {
            args.extend(["--note", note]);
        }
        fixture.json(&args);
    }
    let table = fixture.json(&["stats", "table", "--days", "7", "--group", "item"]);
    assert_eq!(table["group"], "item");
    assert_eq!(table["rows"][0]["key"], item);
    assert_eq!(table["totals"]["minutes"], 40);
    let weeks = fixture.json(&["stats", "table", "--days", "7", "--project", "none"]);
    assert_eq!(weeks["group"], "week");
    assert_eq!(weeks["filter"]["project_id"], "none");
    assert_eq!(weeks["totals"]["sessions"], 2, "the item has no project");
    let lines = fixture.json(&[
        "stats",
        "series",
        "--days",
        "7",
        "--measure",
        "minutes",
        "--split",
        "item",
    ]);
    assert_eq!(lines["split"], "item");
    assert_eq!(lines["series"][0]["key"], item);
    assert_eq!(lines["series"][0]["points"].as_array().unwrap().len(), 7);
    let old_form = fixture.json(&["stats", "--days", "7", "--json"]);
    assert_eq!(old_form["totals"]["minutes"], 40);

    let noted = fixture.json(&["sessions", "list", "--days", "7"]);
    assert_eq!(noted["rows"].as_array().unwrap().len(), 1);
    let found = fixture.json(&[
        "sessions",
        "list",
        "--all",
        "--q",
        "ĀYAT",
        "--min-focus",
        "3",
        "--max-focus",
        "5",
        "--min-minutes",
        "20",
        "--item",
        &item[..8],
    ]);
    assert_eq!(found["rows"][0]["note"], "Āyat review");
    let generic = fixture.json(&[
        "sessions", "list", "--all", "--max", "focus=2", "--limit", "1",
    ]);
    assert_eq!(generic["rows"][0]["minutes"], 10);
    assert!(generic["next"].is_null());
    let bad = fixture.run(&["sessions", "list", "--limit", "0"]);
    assert!(!bad.status.success());
    assert!(stderr(&bad).contains("limit"), "{}", stderr(&bad));
}

#[test]
fn epics_are_added_assigned_reported_and_finished() {
    let fixture = Fixture::new();
    let project = fixture.added("projects", &["project", "add", "Book", "--json"]);
    let epic = fixture.added(
        "epics",
        &[
            "epic",
            "add",
            "Chapter 1",
            "--project",
            &project,
            "--note",
            "Draft",
            "--json",
        ],
    );
    let item = fixture.added(
        "items",
        &[
            "item",
            "add",
            "Write",
            "--project",
            &project,
            "--epic",
            &epic,
            "--json",
        ],
    );
    let logged = fixture.json(&[
        "session",
        "log",
        "--epic",
        &epic,
        "--minutes",
        "20",
        "--json",
    ]);
    let session = &logged["changed"]["sessions"][0];
    assert_eq!(
        (&session["project_id"], &session["epic_id"]),
        (&Value::from(project.as_str()), &Value::from(epic.as_str()))
    );
    fixture.json(&[
        "session",
        "log",
        "--item",
        &item,
        "--minutes",
        "10",
        "--json",
    ]);
    fixture.json(&[
        "session",
        "log",
        "--project",
        &project,
        "--minutes",
        "5",
        "--json",
    ]);

    let listed = fixture.json(&["epic", "list", "--project", &project, "--json"]);
    assert_eq!(listed[0]["total_minutes"], 30);
    let human = fixture.run(&["epic", "list"]);
    let human = String::from_utf8_lossy(&human.stdout);
    assert!(
        human.contains("active") && human.contains("Chapter 1  30 min in 2 sessions"),
        "{human}"
    );
    let table = fixture.json(&["stats", "table", "--days", "1", "--group", "epic"]);
    assert_eq!(table["rows"][0]["key"], epic.as_str());
    assert_eq!(table["rows"][1]["key"], "none");
    assert_eq!(table["rows"][1]["minutes"], 5);
    let outside = fixture.json(&["stats", "--days", "1", "--epic", "none", "--json"]);
    assert_eq!(outside["totals"]["minutes"], 5);

    let removed = fixture.run(&["epic", "remove", &epic]);
    assert!(!removed.status.success());
    assert!(
        stderr(&removed).contains("finish it instead"),
        "{}",
        stderr(&removed)
    );
    let finished = fixture.json(&["epic", "finish", &epic, "--json"]);
    assert!(finished["changed"]["epics"][0]["finished_at"].is_string());
    let reopened = fixture.json(&["epic", "reopen", &epic, "--json"]);
    assert_eq!(reopened["changed"]["epics"][0]["status"], "active");
    let edited = fixture.json(&[
        "epic",
        "edit",
        &epic,
        "--name",
        "One",
        "--no-note",
        "--json",
    ]);
    assert!(edited["changed"]["epics"][0]["note"].is_null());
    let detached = fixture.json(&["item", "edit", &item, "--no-epic", "--json"]);
    assert!(detached["changed"]["items"][0]["epic_id"].is_null());

    let spare = fixture.added(
        "epics",
        &["epic", "add", "Spare", "--project", &project, "--json"],
    );
    let output = fixture.run(&["epic", "remove", &spare]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("removed "));
}

#[test]
fn quick_items_join_the_epic_project_and_backlog_tasks_get_scheduled() {
    let fixture = Fixture::new();
    let project = fixture.added("projects", &["project", "add", "Book", "--json"]);
    let epic = fixture.added(
        "epics",
        &["epic", "add", "Draft", "--project", &project, "--json"],
    );
    let quick = fixture.json(&[
        "item",
        "add",
        "--quick",
        "--epic",
        &epic,
        "write 30m after isha",
        "--json",
    ]);
    let item = &quick["changed"]["items"][0];
    assert_eq!(
        (&item["project_id"], &item["epic_id"], &item["block"]),
        (
            &Value::from(project.as_str()),
            &Value::from(epic.as_str()),
            &Value::from("isha")
        )
    );

    let task = fixture.added(
        "items",
        &[
            "backlog",
            "add",
            "--quick",
            "read notes 20m tomorrow",
            "--project",
            &project,
            "--json",
        ],
    );
    let listed = fixture.json(&["backlog", "list", "--project", &project, "--json"]);
    assert_eq!(listed[0]["title"], "Read notes");
    assert_eq!(listed[0]["planned_minutes"], 20);
    let human = fixture.run(&["backlog", "list"]);
    let human = String::from_utf8_lossy(&human.stdout);
    assert!(human.contains("Read notes  [Book]  20 min"), "{human}");
    let added = fixture.run(&["backlog", "add", "Call"]);
    assert!(String::from_utf8_lossy(&added.stdout).contains("(backlog)"));

    let scheduled = fixture.json(&[
        "backlog",
        "schedule",
        &task[..8],
        "--date",
        "2031-05-15",
        "--block",
        "asr",
        "--json",
    ]);
    assert_eq!(scheduled["changed"]["items"][0]["day"], "2031-05-15");
    let listed = fixture.json(&["backlog", "list", "--json"]);
    assert_eq!(listed.as_array().unwrap().len(), 1);
    let moved = fixture.run(&["backlog", "move", &task]);
    assert!(String::from_utf8_lossy(&moved.stdout).contains("Read notes  (backlog)"));
    let listed = fixture.json(&["backlog", "list", "--json"]);
    assert_eq!(listed.as_array().unwrap().len(), 2);
    let items = fixture.run(&["item", "list"]);
    assert!(String::from_utf8_lossy(&items.stdout).contains("backlog  Call"));
}

#[test]
fn main_stats_item_none_counts_epic_only_work() {
    let fixture = Fixture::new();
    let p = fixture.added("projects", &["project", "add", "Book", "--json"]);
    let e = fixture.added(
        "epics",
        &["epic", "add", "Chapter", "--project", &p, "--json"],
    );
    let task = fixture.added("items", &["item", "add", "Read", "--epic", &e, "--json"]);
    fixture.json(&["session", "log", "--epic", &e, "--minutes", "20", "--json"]);
    fixture.json(&[
        "session",
        "log",
        "--item",
        &task,
        "--minutes",
        "5",
        "--json",
    ]);
    let stats = fixture.json(&["stats", "--days", "1", "--item", "none", "--json"]);
    assert_eq!(stats["totals"]["minutes"], 20);
    assert_eq!(stats["filter"]["item_id"], "none");
}

#[test]
fn repeatable_task_tags_replace_and_clear_the_set() {
    let f = Fixture::new();
    let a = f.added(
        "tags",
        &["tag", "add", "Study", "--color", "#223344", "--json"],
    );
    let b = f.added(
        "tags",
        &["tag", "add", "Reading", "--color", "#334455", "--json"],
    );
    let item = f.json(&["item", "add", "Book", "--tag", &a, "--tag", &b, "--json"]);
    let row = &item["changed"]["items"][0];
    let tags = row["tag_ids"].as_array().unwrap();
    assert_eq!(tags.len(), 2);
    assert!(tags.contains(&Value::from(a.as_str())) && tags.contains(&Value::from(b.as_str())));
    let id = row["id"].as_str().unwrap();
    let edited = f.json(&["item", "edit", id, "--tag", &b, "--json"]);
    assert_eq!(
        edited["changed"]["items"][0]["tag_ids"],
        serde_json::json!([b])
    );
    let cleared = f.json(&["item", "edit", id, "--tag", "--json"]);
    assert_eq!(
        cleared["changed"]["items"][0]["tag_ids"],
        serde_json::json!([])
    );
}
