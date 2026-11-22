//! End-to-end `salah rpc`: NDJSON frames over stdin/stdout against a temp
//! `SALAH_CONFIG`/`SALAH_DATA` and a frozen `SALAH_NOW`.
//!
//! Run with `UPDATE_FIXTURES=1 cargo test --offline --test rpc` to regenerate
//! the golden responses deliberately.

use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/rpc");
const NOW: &str = "2031-05-14T09:30:00+05:00";

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

    fn exchange(&self, lines: &[&str]) -> Vec<Value> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_salah"))
            .env("SALAH_CONFIG", &self.config)
            .env("SALAH_DATA", &self.data)
            .env("XDG_CONFIG_HOME", self.root.join("xdg-config"))
            .env("XDG_DATA_HOME", self.root.join("xdg-data"))
            .env("XDG_CACHE_HOME", self.root.join("xdg-cache"))
            .env("SALAH_NOW", NOW)
            .arg("rpc")
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
