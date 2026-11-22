//! End-to-end `salah rpc`: NDJSON frames over stdin/stdout against a temp
//! `SALAH_CONFIG`/`SALAH_DATA` and a frozen `SALAH_NOW`.
//!
//! The responses are the golden contract fixtures under `tests/fixtures/rpc/`
//! that the frontends code against. Random ids are replaced by `<id:N>` in
//! order of first appearance. Run with
//! `UPDATE_FIXTURES=1 cargo test --offline --test rpc` to regenerate them.

use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/rpc");
/// Inside the Fajr block of 2031-05-14 in the default location.
const NOW: &str = "2031-05-14T09:30:00+05:00";
/// Twenty-five minutes later, still in Fajr.
const LATER: &str = "2031-05-14T09:55:00+05:00";
const DATE: &str = "2031-05-14";

struct Harness {
    root: PathBuf,
    config: PathBuf,
    data: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "salah-rpc-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config.json");
        salah_core::config::save_to(
            &salah_core::config::Config {
                method: "mwl".into(),
                ..salah_core::config::Config::default()
            },
            &config,
        )
        .unwrap();
        Self {
            data: root.join("planner.db"),
            config,
            root,
        }
    }

    fn command(&self, now: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_salah"));
        command
            .env("SALAH_CONFIG", &self.config)
            .env("SALAH_DATA", &self.data)
            .env("XDG_CONFIG_HOME", self.root.join("xdg-config"))
            .env("XDG_DATA_HOME", self.root.join("xdg-data"))
            .env("XDG_CACHE_HOME", self.root.join("xdg-cache"))
            .env("SALAH_NOW", now)
            .arg("rpc");
        command
    }

    /// Writes every line, closes stdin, and collects the responses.
    fn exchange(&self, lines: &[&str]) -> Vec<Value> {
        let mut child = self
            .command(NOW)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        {
            let stdin = child.stdin.as_mut().unwrap();
            for line in lines {
                writeln!(stdin, "{line}").unwrap();
            }
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "salah rpc failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(|line| {
                serde_json::from_str(line)
                    .unwrap_or_else(|error| panic!("not JSON: {error}\nline: {line}"))
            })
            .collect()
    }
}

/// One `salah rpc` process driven request by request, so later requests can
/// use ids returned by earlier ones.
struct Session {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next_id: u64,
}

impl Session {
    fn open(harness: &Harness, now: &str) -> Self {
        let mut child = harness
            .command(now)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut session = Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
            next_id: 0,
        };
        session.request(
            "hello",
            json!({"protocol":[1],"client":"fixtures/1","actor":"test"}),
        );
        session
    }

    /// Sends one request and returns the whole response object.
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let request = json!({"jsonrpc":"2.0","id":self.next_id,"method":method,"params":params});
        writeln!(self.input, "{request}").unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("not JSON: {error}\nline: {line}"))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Records responses as fixtures with stable placeholders for random ids.
#[derive(Default)]
struct Fixtures {
    ids: HashMap<String, String>,
}

impl Fixtures {
    /// Checks `response` against fixture `name` and returns its real result.
    fn check(&mut self, name: &str, response: Value) -> Value {
        let mut stable = response.clone();
        self.normalise(&mut stable);
        compare_fixture(name, &stable);
        response.get("result").cloned().unwrap_or(Value::Null)
    }

    fn normalise(&mut self, value: &mut Value) {
        match value {
            Value::String(text) if is_uuid(text) => {
                let next = format!("<id:{}>", self.ids.len() + 1);
                *text = self.ids.entry(text.clone()).or_insert(next).clone();
            }
            Value::Array(values) => values.iter_mut().for_each(|v| self.normalise(v)),
            Value::Object(map) => {
                *map = std::mem::take(map)
                    .into_iter()
                    .map(|(key, mut value)| {
                        self.normalise(&mut value);
                        let mut key = Value::String(key);
                        self.normalise(&mut key);
                        (key.as_str().unwrap().to_owned(), value)
                    })
                    .collect();
            }
            _ => {}
        }
    }
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

fn id(result: &Value, kind: &str) -> String {
    result["changed"][kind][0]["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn compare_fixture(name: &str, value: &Value) {
    let path = Path::new(FIXTURES).join(format!("{name}.json"));
    let rendered = format!("{}\n", serde_json::to_string_pretty(value).unwrap());
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        fs::create_dir_all(FIXTURES).unwrap();
        fs::write(&path, rendered).unwrap();
        return;
    }
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("missing fixture {}: {error}", path.display())),
    )
    .unwrap();
    assert_eq!(
        value, &expected,
        "fixture {name} changed; run \
         `UPDATE_FIXTURES=1 cargo test --offline --test rpc` if intentional"
    );
}

#[test]
fn hello_status_and_day_are_ndjson_and_match_fixtures() {
    let harness = Harness::new();
    let responses = harness.exchange(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"hello","params":{"protocol":[1],"client":"integration/1","actor":"test"}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"status.get","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"day.get","params":{"date":"2031-05-14"}}"#,
    ]);
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[0]["result"]["protocol"], 1);
    assert_eq!(responses[1]["id"], 2);
    assert_eq!(responses[2]["id"], 3);
    compare_fixture("hello", &responses[0]);
    compare_fixture("status", &responses[1]);
    compare_fixture("day", &responses[2]);
}

/// One fixture per tracking method, in the order a user would meet them.
#[test]
fn tracking_methods_match_fixtures() {
    let harness = Harness::new();
    let mut fixtures = Fixtures::default();
    let mut rpc = Session::open(&harness, NOW);
    let mut check = |rpc: &mut Session, name: &str, method: &str, params: Value| {
        let response = rpc.request(method, params);
        fixtures.check(name, response)
    };

    check(&mut rpc, "metrics_list", "metrics.list", json!({}));
    let pages = id(
        &check(
            &mut rpc,
            "metric_add",
            "metric.add",
            json!({"op_id":"op-metric-add","name":"Pages","unit":"pages","kind":"count"}),
        ),
        "metrics",
    );
    check(
        &mut rpc,
        "metric_edit",
        "metric.edit",
        json!({"op_id":"op-metric-edit","id":pages,"name":"Pages read"}),
    );
    let tag = id(
        &check(
            &mut rpc,
            "tag_add",
            "tag.add",
            json!({"op_id":"op-tag-add","name":"Deen","color":"#2e7d32"}),
        ),
        "tags",
    );
    check(
        &mut rpc,
        "tag_edit",
        "tag.edit",
        json!({"op_id":"op-tag-edit","id":tag,"name":"Faith"}),
    );
    check(&mut rpc, "tags_list", "tags.list", json!({}));
    let quran = id(
        &check(
            &mut rpc,
            "project_add",
            "project.add",
            json!({"op_id":"op-project-add","name":"Quran","color":"#4c8bf5","tag_ids":[tag]}),
        ),
        "projects",
    );
    check(
        &mut rpc,
        "project_edit",
        "project.edit",
        json!({
            "op_id":"op-project-edit","id":quran,"note":"Daily recitation",
            "metrics":[{"metric_id":"focus","required":true},{"metric_id":"quality","required":true},
                       {"metric_id":pages,"required":false}]
        }),
    );
    check(&mut rpc, "projects_list", "projects.list", json!({}));
    check(
        &mut rpc,
        "item_parse",
        "item.parse",
        json!({"text":"read 20m after fajr daily"}),
    );
    check(
        &mut rpc,
        "item_preview",
        "item.preview",
        json!({
            "block":"fajr","repeat":"weekly","weekdays":["mon","wed","fri"],"planned_minutes":20,"date":DATE
        }),
    );
    let read = id(
        &check(
            &mut rpc,
            "item_add",
            "item.add",
            json!({
                "op_id":"op-item-add","title":"Read","block":"fajr","repeat":"daily",
                "project_id":quran,"planned_minutes":30
            }),
        ),
        "items",
    );
    check(
        &mut rpc,
        "item_edit",
        "item.edit",
        json!({"op_id":"op-item-edit","id":read,"title":"Read Quran","expected_version":1}),
    );
    let walk = id(
        &check(
            &mut rpc,
            "item_add_quick",
            "item.add",
            json!({"op_id":"op-item-add-quick","quick_text":"walk 15m after asr"}),
        ),
        "items",
    );
    check(
        &mut rpc,
        "item_move",
        "item.move",
        json!({"op_id":"op-item-move","id":walk,"block":"fajr","after_id":read}),
    );
    check(&mut rpc, "items_list", "items.list", json!({}));
    check(
        &mut rpc,
        "session_start",
        "session.start",
        json!({"op_id":"op-session-start","item_id":read}),
    );
    check(&mut rpc, "status_running", "status.get", json!({}));
    drop(rpc);

    let mut rpc = Session::open(&harness, LATER);
    check(
        &mut rpc,
        "session_stop_missing_metrics",
        "session.stop",
        json!({"op_id":"op-session-stop-missing","values":{"focus":4}}),
    );
    check(
        &mut rpc,
        "session_stop",
        "session.stop",
        json!({
            "op_id":"op-session-stop","values":{"focus":4,"quality":5,(pages.as_str()):6},"note":"Calm"
        }),
    );
    let logged = id(
        &check(
            &mut rpc,
            "session_log",
            "session.log",
            json!({
                "op_id":"op-session-log","project_id":quran,"minutes":15,"values":{"focus":3,"quality":4}
            }),
        ),
        "sessions",
    );
    check(
        &mut rpc,
        "session_edit",
        "session.edit",
        json!({"op_id":"op-session-edit","id":logged,"minutes":20,"note":"Review"}),
    );
    check(
        &mut rpc,
        "session_log_tick",
        "session.log",
        json!({"op_id":"op-session-tick","item_id":walk}),
    );
    check(
        &mut rpc,
        "prayer_mark",
        "prayer.mark",
        json!({"op_id":"op-prayer-mark","prayer":"fajr"}),
    );
    check(&mut rpc, "day_tracking", "day.get", json!({}));
    check(&mut rpc, "stats", "stats.get", json!({"days":7}));
    check(
        &mut rpc,
        "stats_by_tag",
        "stats.get",
        json!({"days":7,"tag_id":tag}),
    );
    check(
        &mut rpc,
        "session_remove",
        "session.remove",
        json!({"op_id":"op-session-remove","id":logged}),
    );
    check(
        &mut rpc,
        "tag_remove",
        "tag.remove",
        json!({"op_id":"op-tag-remove","id":tag}),
    );
    let removed = check(
        &mut rpc,
        "item_remove",
        "item.remove",
        json!({"op_id":"op-item-remove","id":walk}),
    );
    let replay = rpc.request(
        "item.remove",
        json!({"op_id":"op-item-remove","id":"other"}),
    );
    assert_eq!(
        replay["result"], removed,
        "a replayed op_id returns the stored result"
    );
}

#[test]
fn malformed_and_unhandshaken_requests_get_jsonrpc_errors() {
    let harness = Harness::new();
    let responses = harness.exchange(&[
        "{not json",
        r#"{"jsonrpc":"2.0","id":2,"method":"status.get","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"not.a.method","params":{}}"#,
    ]);
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["error"]["code"], -32700);
    assert_eq!(responses[1]["error"]["code"], -32600);
    // Without a successful hello, even an unknown method is reported as a
    // handshake fault, not method-not-found.
    assert_eq!(responses[2]["error"]["code"], -32600);
}
