//! Golden CLI contract fixtures for the desktop app and the bar widget.
//!
//! Each test runs the real `salah` binary against an isolated temporary
//! `SALAH_CONFIG` and `SALAH_DATA`. Values the real clock or random ids make
//! volatile are replaced with stable placeholders before the JSON is compared
//! with `tests/fixtures/contract/<name>.json`.
//!
//! Run with `UPDATE_FIXTURES=1 cargo test --offline --test contract` to
//! regenerate the fixtures from the current binary.

use chrono::{DateTime, Duration, FixedOffset, NaiveDate, Utc};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/contract");
const CONTRACT_TZ_HOURS: i32 = 5;
const DATE: &str = "2031-05-14";

#[derive(Clone, Copy, Default)]
struct Norm {
    /// `status --json` embeds values that depend on the current time of day.
    status_timing: bool,
    /// `item list --json` reports the current block.
    list_timing: bool,
    /// Timer entries are recorded in whichever block is current.
    timer_block: bool,
    /// `stats --days` dates are relative to its `to` day, not a fixed date.
    stats_days: bool,
}

struct Harness {
    root: PathBuf,
    config: PathBuf,
    data: PathBuf,
}

impl Harness {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "salah-contract-{}-{}-{name}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let harness = Self {
            config: root.join("config.json"),
            data: root.join("planner.db"),
            root,
        };
        // The method keeps status and day on the offline calculation path and
        // away from the official Muslim Board cache.
        let output = harness.run(&["config", "set", "--method", "mwl"]);
        assert_success(&output);
        harness
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_salah"))
            .env("SALAH_CONFIG", &self.config)
            .env("SALAH_DATA", &self.data)
            .env("XDG_CONFIG_HOME", self.root.join("xdg-config"))
            .env("XDG_DATA_HOME", self.root.join("xdg-data"))
            .env("XDG_CACHE_HOME", self.root.join("xdg-cache"))
            .args(args)
            .output()
            .unwrap()
    }

    fn json(&self, args: &[&str]) -> Value {
        let output = self.run(args);
        assert_success(&output);
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "not JSON from {args:?}: {error}\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        })
    }

    fn snapshot(&self, name: &str, args: &[&str], norm: Norm) -> Value {
        let value = self.json(args);
        self.record(name, &value, norm);
        value
    }

    fn record(&self, name: &str, value: &Value, norm: Norm) {
        let mut normalised = value.clone();
        normalise_root(&mut normalised, norm);
        compare_fixture(name, &normalised);
    }

    fn error(&self, name: &str, args: &[&str]) {
        let output = self.run(args);
        assert!(
            !output.status.success(),
            "expected failure from {args:?}\nstdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.starts_with("salah: "),
            "stderr must start with 'salah: ', got {stderr:?}"
        );
        let message = stderr.trim_end_matches(['\r', '\n']);
        compare_fixture(name, &json!({ "stderr": message }));
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed with {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn compare_fixture(name: &str, value: &Value) {
    let path = Path::new(FIXTURES).join(format!("{name}.json"));
    let rendered = format!("{}\n", serde_json::to_string_pretty(value).unwrap());
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        fs::create_dir_all(FIXTURES).unwrap();
        fs::write(&path, rendered).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "missing fixture {}: {error}; run \
             `UPDATE_FIXTURES=1 cargo test --offline --test contract`",
            path.display()
        )
    });
    let expected: Value = serde_json::from_str(&expected)
        .unwrap_or_else(|error| panic!("invalid fixture {}: {error}", path.display()));
    assert_eq!(
        value, &expected,
        "fixture {name} changed; run \
         `UPDATE_FIXTURES=1 cargo test --offline --test contract` if the \
         change is intentional"
    );
}

fn contract_today() -> NaiveDate {
    let offset = FixedOffset::east_opt(CONTRACT_TZ_HOURS * 3600).unwrap();
    Utc::now().with_timezone(&offset).date_naive()
}

struct Context {
    today: NaiveDate,
    /// When set, every date is written relative to this day (`<to-3>`).
    anchor: Option<NaiveDate>,
    ids: HashMap<String, String>,
}

fn normalise_root(value: &mut Value, norm: Norm) {
    let anchor = norm
        .stats_days
        .then(|| value.get("to").and_then(Value::as_str).and_then(parse_date))
        .flatten();
    let mut context = Context {
        today: contract_today(),
        anchor,
        ids: HashMap::new(),
    };
    normalise_value(value, &mut context);
    if norm.status_timing {
        normalise_status_timing(value);
    }
    if norm.list_timing {
        normalise_list_timing(value);
    }
    if norm.timer_block {
        normalise_timer_block(value);
    }
}

fn normalise_value(value: &mut Value, context: &mut Context) {
    match value {
        Value::String(text) => {
            if let Some(replacement) = normalise_string(text, context) {
                *text = replacement;
            }
        }
        Value::Array(items) => {
            for item in items {
                normalise_value(item, context);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                normalise_value(item, context);
            }
        }
        _ => {}
    }
}

fn normalise_string(text: &str, context: &mut Context) -> Option<String> {
    if text.len() == 36 && uuid::Uuid::parse_str(text).is_ok() {
        let next = context.ids.len() + 1;
        return Some(
            context
                .ids
                .entry(text.to_string())
                .or_insert_with(|| format!("<id:{next}>"))
                .clone(),
        );
    }
    if DateTime::parse_from_rfc3339(text).is_ok() {
        return Some("<instant>".into());
    }
    if let Some(date) = parse_date(text) {
        return Some(date_placeholder(date, text, context));
    }
    if text.len() == 5
        && text.as_bytes()[2] == b':'
        && text
            .bytes()
            .enumerate()
            .all(|(index, byte)| index == 2 || byte.is_ascii_digit())
    {
        return Some("<HH:MM>".into());
    }
    None
}

fn parse_date(text: &str) -> Option<NaiveDate> {
    if text.len() == 10 {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
    } else {
        None
    }
}

fn date_placeholder(date: NaiveDate, text: &str, context: &Context) -> String {
    if let Some(anchor) = context.anchor {
        return relative_placeholder("to", date.signed_duration_since(anchor).num_days());
    }
    let offset = date.signed_duration_since(context.today).num_days();
    // Explicit fixture dates stay literal; only the clock-relative near days
    // are replaced.
    if (-1..=1).contains(&offset) {
        relative_placeholder("today", offset)
    } else {
        text.to_string()
    }
}

fn relative_placeholder(base: &str, offset: i64) -> String {
    if offset == 0 {
        format!("<{base}>")
    } else {
        format!("<{base}{offset:+}>")
    }
}

fn normalise_status_timing(value: &mut Value) {
    let Some(status) = value.as_object_mut() else {
        return;
    };
    status.insert("label".into(), json!("<duration>"));
    if let Some(next) = status.get_mut("next").and_then(Value::as_object_mut) {
        next.insert("id".into(), json!("<boundary>"));
        next.insert("name".into(), json!("<name>"));
        next.insert("kind".into(), json!("<kind>"));
        next.insert("time".into(), json!("<HH:MM>"));
        next.insert("remaining".into(), json!("<duration>"));
        next.insert("remaining_short".into(), json!("<duration>"));
        next.insert("remaining_seconds".into(), json!(0));
    }
    if let Some(block) = status
        .get_mut("current_block")
        .and_then(Value::as_object_mut)
    {
        block.insert("sleeping".into(), json!(false));
        if let Some(key) = block.get_mut("block").and_then(Value::as_object_mut) {
            key.insert("start".into(), json!("<boundary>"));
        }
        for point in ["start", "end"] {
            if let Some(point) = block.get_mut(point).and_then(Value::as_object_mut) {
                point.insert("id".into(), json!("<boundary>"));
                point.insert("name".into(), json!("<name>"));
                point.insert("kind".into(), json!("<kind>"));
                point.insert("time".into(), json!("<HH:MM>"));
            }
        }
    }
    if let Some(planner) = status.get_mut("planner").and_then(Value::as_object_mut) {
        planner.insert("block".into(), json!("<boundary>"));
        if let Some(planning) = planner
            .get_mut("planning_block")
            .and_then(Value::as_object_mut)
        {
            planning.insert("start".into(), json!("<boundary>"));
        }
    }
    for key in ["prayers", "boundaries"] {
        if let Some(rows) = status.get_mut(key).and_then(Value::as_array_mut) {
            for row in rows {
                if let Some(row) = row.as_object_mut() {
                    row.insert("passed".into(), json!(false));
                    row.insert("is_next".into(), json!(false));
                    row.insert("current".into(), json!(false));
                }
            }
        }
    }
}

fn normalise_list_timing(value: &mut Value) {
    let Some(list) = value.as_object_mut() else {
        return;
    };
    list.insert("block".into(), json!("<boundary>"));
    if let Some(planning) = list
        .get_mut("planning_block")
        .and_then(Value::as_object_mut)
    {
        planning.insert("start".into(), json!("<boundary>"));
    }
}

fn normalise_timer_block(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                normalise_timer_block(item);
            }
        }
        Value::Object(map) => {
            if let Some(block) = map.get_mut("block").and_then(Value::as_object_mut) {
                block.insert("day".into(), json!("<today>"));
                block.insert("start".into(), json!("<boundary>"));
            }
        }
        _ => {}
    }
}

fn item_add(harness: &Harness, extra: &[&str], title: &str) -> Value {
    let mut args = vec!["item", "add", "--json"];
    args.extend_from_slice(extra);
    args.push("--");
    args.push(title);
    harness.json(&args)
}

fn id(item: &Value) -> &str {
    item["id"].as_str().unwrap()
}

fn day_text(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

#[test]
fn status_and_today_list_shapes() {
    let harness = Harness::new("status");
    harness.snapshot(
        "status",
        &["status", "--json"],
        Norm {
            status_timing: true,
            ..Norm::default()
        },
    );
    harness.snapshot(
        "item_list",
        &["item", "list", "--json"],
        Norm {
            list_timing: true,
            ..Norm::default()
        },
    );
}

#[test]
fn config_show_shape() {
    let harness = Harness::new("config");
    harness.snapshot("config_show", &["config", "show"], Norm::default());
}

#[test]
fn day_plan_with_items_and_marks() {
    let harness = Harness::new("day");
    item_add(
        &harness,
        &[
            "--date",
            DATE,
            "--block",
            "fajr",
            "--repeat",
            "daily",
            "--measure",
            "minutes",
            "--target",
            "30",
        ],
        "Read Quran",
    );
    item_add(
        &harness,
        &[
            "--date",
            "2031-05-12",
            "--block",
            "asr",
            "--repeat",
            "weekly",
            "--days",
            "mon,wed",
            "--measure",
            "count",
            "--target",
            "30",
        ],
        "Pushups",
    );
    item_add(
        &harness,
        &["--date", DATE, "--block", "isha"],
        "No target task",
    );
    harness.json(&["prayer", "mark", "fajr", "--date", DATE, "--json"]);
    harness.snapshot("day", &["day", "--date", DATE, "--json"], Norm::default());
}

#[test]
fn item_add_variants() {
    let harness = Harness::new("add");
    harness.record(
        "item_add_plain",
        &item_add(&harness, &["--date", DATE, "--block", "fajr"], "Plain task"),
        Norm::default(),
    );
    harness.record(
        "item_add_quick",
        &item_add(
            &harness,
            &["--quick", "--date", DATE],
            "read 20m after isha daily",
        ),
        Norm::default(),
    );
    harness.record(
        "item_add_block",
        &item_add(&harness, &["--date", DATE, "--block", "asr"], "Block task"),
        Norm::default(),
    );
    harness.record(
        "item_add_daily",
        &item_add(
            &harness,
            &["--date", DATE, "--block", "dhuhr", "--repeat", "daily"],
            "Daily task",
        ),
        Norm::default(),
    );
    harness.record(
        "item_add_weekdays",
        &item_add(
            &harness,
            &[
                "--date",
                "2031-05-12",
                "--block",
                "maghrib",
                "--repeat",
                "weekdays",
            ],
            "Weekday task",
        ),
        Norm::default(),
    );
    harness.record(
        "item_add_weekly_days",
        &item_add(
            &harness,
            &[
                "--date",
                "2031-05-12",
                "--block",
                "isha",
                "--repeat",
                "weekly",
                "--days",
                "mon,wed,fri",
            ],
            "Weekly task",
        ),
        Norm::default(),
    );
    harness.record(
        "item_add_count_target",
        &item_add(
            &harness,
            &[
                "--date",
                DATE,
                "--block",
                "fajr",
                "--measure",
                "count",
                "--target",
                "10",
            ],
            "Count task",
        ),
        Norm::default(),
    );
    harness.record(
        "item_add_date",
        &item_add(
            &harness,
            &["--date", "2031-06-01", "--block", "fajr"],
            "Dated task",
        ),
        Norm::default(),
    );
}

#[test]
fn item_parse_variants() {
    let harness = Harness::new("parse");
    harness.snapshot(
        "item_parse",
        &["item", "parse", "--json", "--", "read 20m after isha daily"],
        Norm::default(),
    );
    harness.snapshot(
        "item_parse_weekdays",
        &[
            "item",
            "parse",
            "--json",
            "--",
            "pushups 30 times every mon, wed and fri",
        ],
        Norm::default(),
    );
}

#[test]
fn item_edit_variants() {
    let harness = Harness::new("edit");
    let item = item_add(
        &harness,
        &[
            "--date",
            DATE,
            "--block",
            "fajr",
            "--measure",
            "minutes",
            "--target",
            "20",
        ],
        "Original task",
    );
    let id = id(&item).to_string();

    harness.snapshot(
        "item_edit_title",
        &["item", "edit", "--json", &id, "--title=Renamed task"],
        Norm::default(),
    );
    harness.snapshot(
        "item_edit_block",
        &["item", "edit", "--json", &id, "--block", "maghrib"],
        Norm::default(),
    );
    harness.snapshot(
        "item_edit_repeat",
        &[
            "item", "edit", "--json", &id, "--repeat", "weekly", "--days", "mon,wed",
        ],
        Norm::default(),
    );
    harness.snapshot(
        "item_edit_target",
        &["item", "edit", "--json", &id, "--target", "45"],
        Norm::default(),
    );
    harness.snapshot(
        "item_edit_no_target",
        &["item", "edit", "--json", &id, "--no-target"],
        Norm::default(),
    );
}

#[test]
fn item_move_changes_block_and_position() {
    let harness = Harness::new("move");
    item_add(&harness, &["--date", DATE, "--block", "fajr"], "First task");
    let second = item_add(
        &harness,
        &["--date", DATE, "--block", "fajr"],
        "Second task",
    );
    let id = id(&second).to_string();
    harness.snapshot(
        "item_move",
        &[
            "item", "move", "--json", &id, "--block", "asr", "--index", "0",
        ],
        Norm::default(),
    );
}

#[test]
fn item_done_and_undo() {
    let harness = Harness::new("done");
    let item = item_add(&harness, &["--date", DATE, "--block", "dhuhr"], "Tick task");
    let id = id(&item).to_string();
    harness.snapshot(
        "item_done",
        &["item", "done", "--json", &id, "--date", DATE],
        Norm::default(),
    );
    harness.snapshot(
        "item_undo",
        &["item", "undo", "--json", &id, "--date", DATE],
        Norm::default(),
    );
}

#[test]
fn item_log_positive_negative_and_minutes() {
    let harness = Harness::new("log");
    let timed = item_add(
        &harness,
        &["--date", DATE, "--block", "dhuhr", "--measure", "minutes"],
        "Timed task",
    );
    let timed_id = id(&timed).to_string();
    harness.snapshot(
        "item_log_positive",
        &["item", "log", "--json", &timed_id, "20", "--date", DATE],
        Norm::default(),
    );
    harness.snapshot(
        "item_log_negative",
        &["item", "log", "--json", &timed_id, "-5", "--date", DATE],
        Norm::default(),
    );

    let counted = item_add(
        &harness,
        &["--date", DATE, "--block", "asr", "--measure", "count"],
        "Counted task",
    );
    let counted_id = id(&counted).to_string();
    harness.snapshot(
        "item_log_count",
        &["item", "log", "--json", &counted_id, "3", "--date", DATE],
        Norm::default(),
    );
    harness.snapshot(
        "item_log_minutes_positive",
        &[
            "item",
            "log",
            "--json",
            "--minutes",
            &counted_id,
            "15",
            "--date",
            DATE,
        ],
        Norm::default(),
    );
    harness.snapshot(
        "item_log_minutes_negative",
        &[
            "item",
            "log",
            "--json",
            "--minutes",
            &counted_id,
            "-5",
            "--date",
            DATE,
        ],
        Norm::default(),
    );
}

#[test]
fn item_start_and_stop() {
    let harness = Harness::new("timer");
    let item = item_add(&harness, &["--date", DATE], "Timer task");
    let id = id(&item).to_string();
    let timer = Norm {
        timer_block: true,
        ..Norm::default()
    };
    harness.snapshot("item_start", &["item", "start", "--json", &id], timer);
    harness.snapshot("item_stop_id", &["item", "stop", "--json", &id], timer);
    harness.json(&["item", "start", "--json", &id]);
    harness.snapshot("item_stop", &["item", "stop", "--json"], timer);
}

#[test]
fn item_remove_returns_tombstone() {
    let harness = Harness::new("remove");
    let item = item_add(&harness, &["--date", DATE], "Remove me");
    let id = id(&item).to_string();
    harness.snapshot(
        "item_remove",
        &["item", "remove", "--json", &id],
        Norm::default(),
    );
}

#[test]
fn prayer_mark_and_unmark() {
    let harness = Harness::new("prayer");
    harness.snapshot(
        "prayer_mark",
        &["prayer", "mark", "--json", "fajr", "--date", DATE],
        Norm::default(),
    );
    harness.snapshot(
        "prayer_unmark",
        &["prayer", "unmark", "--json", "fajr", "--date", DATE],
        Norm::default(),
    );
}

#[test]
fn stats_over_an_explicit_range() {
    let harness = Harness::new("stats-range");
    let daily = item_add(
        &harness,
        &[
            "--date",
            "2020-01-01",
            "--block",
            "fajr",
            "--repeat",
            "daily",
            "--measure",
            "minutes",
            "--target",
            "30",
        ],
        "Read Quran",
    );
    let daily_id = id(&daily).to_string();
    let weekly = item_add(
        &harness,
        &[
            "--date",
            "2020-01-01",
            "--block",
            "asr",
            "--repeat",
            "weekly",
            "--days",
            "mon,wed",
            "--measure",
            "count",
            "--target",
            "5",
        ],
        "Pushups",
    );
    let weekly_id = id(&weekly).to_string();
    let once = item_add(
        &harness,
        &["--date", "2020-01-08", "--block", "isha"],
        "Call mum",
    );
    let once_id = id(&once).to_string();

    harness.json(&[
        "item",
        "log",
        "--json",
        &daily_id,
        "20",
        "--date",
        "2020-01-06",
    ]);
    harness.json(&["item", "done", "--json", &daily_id, "--date", "2020-01-07"]);
    harness.json(&[
        "item",
        "log",
        "--json",
        &weekly_id,
        "3",
        "--date",
        "2020-01-08",
    ]);
    harness.json(&["item", "done", "--json", &once_id, "--date", "2020-01-08"]);
    harness.json(&["prayer", "mark", "--json", "fajr", "--date", "2020-01-06"]);
    harness.json(&["prayer", "mark", "--json", "asr", "--date", "2020-01-08"]);

    harness.snapshot(
        "stats_range",
        &[
            "stats",
            "--from",
            "2020-01-06",
            "--to",
            "2020-01-12",
            "--json",
        ],
        Norm::default(),
    );
}

#[test]
fn stats_for_days_ending_today() {
    let harness = Harness::new("stats-days");
    let to = harness.json(&["stats", "--days", "1", "--json"])["to"]
        .as_str()
        .and_then(parse_date)
        .expect("stats returns a date");
    let at = |offset: i64| day_text(to.checked_add_signed(Duration::days(offset)).unwrap());

    let daily = item_add(
        &harness,
        &[
            "--date",
            &at(-4),
            "--block",
            "fajr",
            "--repeat",
            "daily",
            "--measure",
            "minutes",
            "--target",
            "30",
        ],
        "Read Quran",
    );
    let daily_id = id(&daily).to_string();
    harness.json(&["item", "log", "--json", &daily_id, "15", "--date", &at(-2)]);
    harness.json(&["item", "done", "--json", &daily_id, "--date", &at(-1)]);
    harness.json(&["prayer", "mark", "--json", "fajr", "--date", &at(-3)]);

    harness.snapshot(
        "stats_days",
        &["stats", "--days", "7", "--json"],
        Norm {
            stats_days: true,
            ..Norm::default()
        },
    );
}

#[test]
fn export_and_import() {
    let source = Harness::new("export");
    let item = item_add(
        &source,
        &[
            "--date",
            DATE,
            "--block",
            "fajr",
            "--measure",
            "minutes",
            "--target",
            "30",
        ],
        "Exported task",
    );
    let id = id(&item).to_string();
    source.json(&["item", "log", "--json", &id, "20", "--date", DATE]);
    source.json(&["prayer", "mark", "--json", "fajr", "--date", DATE]);

    let output = source.run(&["export"]);
    assert_success(&output);
    let exported: Value = serde_json::from_slice(&output.stdout).unwrap();
    source.record("export", &exported, Norm::default());

    let export_path = source.root.join("export.json");
    fs::write(&export_path, &output.stdout).unwrap();
    let export_path = export_path.to_str().unwrap();

    let target = Harness::new("import");
    target.snapshot(
        "import",
        &["import", export_path, "--json"],
        Norm::default(),
    );
    target.snapshot(
        "import_again",
        &["import", export_path, "--json"],
        Norm::default(),
    );
}

#[test]
fn error_messages_are_stable() {
    let harness = Harness::new("errors");
    let item = item_add(&harness, &["--date", DATE], "Error task");
    let id = id(&item).to_string();

    harness.error("error_unknown_id", &["item", "done", "deadbeef", "--json"]);
    harness.error(
        "error_weekly_without_days",
        &[
            "item",
            "add",
            "--repeat",
            "weekly",
            "--json",
            "--",
            "Weekly task",
        ],
    );
    harness.error(
        "error_sleep_block",
        &[
            "item",
            "add",
            "--block",
            "sleep",
            "--json",
            "--",
            "Sleep task",
        ],
    );
    harness.error("error_zero_log", &["item", "log", "--json", &id, "0"]);
    harness.error("error_bad_date", &["day", "--date", "2026-13-40", "--json"]);
}
