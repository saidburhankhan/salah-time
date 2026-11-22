use salah_core::config::Config;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    root: PathBuf,
    config: PathBuf,
    data: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "salah-planner-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
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
        let data = root.join("planner.db");
        Self { root, config, data }
    }

    fn run(&self, args: &[&str]) -> Output {
        run(&self.config, &self.data, args)
    }

    fn json(&self, args: &[&str]) -> Value {
        let output = self.run(args);
        assert!(output.status.success(), "{}", stderr(&output));
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn run(config: &Path, data: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_salah"))
        .env("SALAH_CONFIG", config)
        .env("SALAH_DATA", data)
        .args(args)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn add(fixture: &Fixture, extra: &[&str]) -> Value {
    let mut args = vec!["item", "add", "Call", "the", "bank"];
    args.extend_from_slice(extra);
    args.extend(["--json"]);
    fixture.json(&args)
}

fn contains_id(list: &Value, id: &str) -> bool {
    ["current", "upcoming"].iter().any(|key| {
        list[*key]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == id)
    })
}

#[test]
fn item_lifecycle_is_visible_in_today() {
    let fixture = Fixture::new();
    let item = add(&fixture, &[]);
    let id = item["id"].as_str().unwrap();
    assert!(contains_id(&fixture.json(&["item", "list", "--json"]), id));

    fixture.json(&["item", "done", id, "--json"]);
    let list = fixture.json(&["item", "list", "--json"]);
    assert_eq!(
        list["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
            .unwrap()["state"],
        "done"
    );

    assert_eq!(fixture.json(&["item", "undo", id, "--json"])["undone"], 1);
    let list = fixture.json(&["item", "list", "--json"]);
    assert_eq!(
        list["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
            .unwrap()["state"],
        "open"
    );

    fixture.json(&["item", "remove", id, "--json"]);
    assert!(!contains_id(&fixture.json(&["item", "list", "--json"]), id));
}

#[test]
#[ignore = "legacy measured items are replaced by session metrics in S3"]
fn measured_items_log_and_check_items_reject_logs() {
    let fixture = Fixture::new();
    let measured = add(&fixture, &["--measure", "minutes"]);
    let id = measured["id"].as_str().unwrap();
    fixture.json(&["item", "log", id, "15", "--json"]);
    let list = fixture.json(&["item", "list", "--json"]);
    assert_eq!(
        list["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
            .unwrap()["minutes"],
        15
    );

    // Check items record time too, and a negative value subtracts. The
    // widget and app send "-5" as its own argument, so it must not be read as
    // a flag.
    let check = add(&fixture, &[]);
    let check_id = check["id"].as_str().unwrap();
    fixture.json(&["item", "log", "--json", check_id, "20"]);
    fixture.json(&["item", "log", "--json", check_id, "-5"]);
    let list = fixture.json(&["item", "list", "--json"]);
    let row = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == check_id)
        .unwrap()
        .clone();
    assert_eq!(row["minutes"], 15);

    let output = fixture.run(&["item", "log", check_id, "0"]);
    assert!(!output.status.success());
    assert!(stderr(&output).starts_with("salah: "));
}

#[test]
fn invalid_inputs_do_not_create_items() {
    let fixture = Fixture::new();
    for args in [
        vec!["item", "done", "unknown"],
        vec!["item", "add", "Nap", "--block", "sleep"],
        vec!["item", "add", "", "--json"],
    ] {
        let output = fixture.run(&args);
        assert!(!output.status.success(), "unexpected success: {args:?}");
    }
    assert!(
        fixture.json(&["export"])["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn unknown_ids_fail() {
    // Ambiguous prefixes are covered by planner unit tests, which control ids.
    let fixture = Fixture::new();
    add(&fixture, &[]);
    let unknown = fixture.run(&["item", "done", "not-an-id"]);
    assert!(!unknown.status.success());
    assert!(stderr(&unknown).contains("no item"));
}

#[test]
fn export_import_round_trip_is_idempotent() {
    let source = Fixture::new();
    let item = add(&source, &["--measure", "count"]);
    source.json(&["item", "log", item["id"].as_str().unwrap(), "3", "--json"]);
    let export_path = source.root.join("data.json");
    let output = source.run(&["export", "--output", export_path.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));

    let target = Fixture::new();
    let first = target.json(&["import", export_path.to_str().unwrap(), "--json"]);
    assert_eq!(first["added"], 2);
    let second = target.json(&["import", export_path.to_str().unwrap(), "--json"]);
    assert_eq!(second["added"], 0);

    let original = source.json(&["export"]);
    let imported = target.json(&["export"]);
    assert_eq!(original["items"], imported["items"]);
    assert_eq!(original["entries"], imported["entries"]);

    let refused = source.run(&["export", "--output", export_path.to_str().unwrap()]);
    assert!(!refused.status.success());
    let replaced = source.run(&[
        "export",
        "--output",
        export_path.to_str().unwrap(),
        "--force",
    ]);
    assert!(replaced.status.success(), "{}", stderr(&replaced));
}

#[test]
fn status_attaches_planner_and_survives_store_failure() {
    let fixture = Fixture::new();
    let status = fixture.json(&["status", "--json"]);
    assert!(status["planner"].is_object());
    assert!(status["planner"]["open"].is_number());

    let regular = fixture.root.join("regular-file");
    std::fs::write(&regular, b"x").unwrap();
    let bad_data = regular.join("planner.db");
    let output = run(&fixture.config, &bad_data, &["status", "--json"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let status: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(status["planner"].is_null());
    assert!(status["planner_error"].is_string());
}

#[test]
#[ignore = "legacy count and signed entry behavior is removed"]
fn timers_stop_by_item_and_count_items_adjust_time_separately() {
    let fixture = Fixture::new();
    let counter = add(&fixture, &["--measure", "count"]);
    let id = counter["id"].as_str().unwrap();
    fixture.json(&["item", "start", "--json", id]);
    // The app stops a specific item's timer by id.
    let stopped = fixture.json(&["item", "stop", "--json", id]);
    assert_eq!(stopped.as_array().unwrap().len(), 1);
    assert_eq!(stopped[0]["item_id"], id);
    fixture.json(&["item", "log", "--json", id, "2"]);
    fixture.json(&["item", "log", "--json", "--minutes", id, "15"]);
    fixture.json(&["item", "log", "--json", "--minutes", id, "-5"]);
    let list = fixture.json(&["item", "list", "--json"]);
    let row = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap()
        .clone();
    assert_eq!(
        (row["count"].as_i64(), row["minutes"].as_i64()),
        (Some(2), Some(10))
    );
}
