//! The human CLI end to end: every tracking subcommand runs the real binary
//! against a temporary store and goes through the protocol dispatcher.

use salah_core::config::Config;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

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
fn a_timed_session_asks_for_the_project_metrics() {
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
    assert_eq!(status["prompt"].as_array().unwrap().len(), 2);

    let missing = fixture.run_at(LATER, &["session", "stop", "--value", "focus=4"]);
    assert!(!missing.status.success());
    assert!(stderr(&missing).contains("missing required metrics: quality"));
    let stopped = fixture.json_at(
        LATER,
        &[
            "session",
            "stop",
            "--value",
            "focus=4",
            "--value",
            "quality=5",
            "--note",
            "calm",
            "--json",
        ],
    );
    assert_eq!(stopped["changed"]["sessions"][0]["minutes"], 25);

    let tick = fixture.json(&[
        "session",
        "log",
        "--project",
        &project,
        "--value",
        "focus=3",
        "--value",
        "quality=3",
        "--json",
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
