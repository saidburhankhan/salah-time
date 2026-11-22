//! End-to-end tests for `salah serve`: the protocol-v1 daemon.
//!
//! Every test uses a fresh temporary runtime/config/data directory and a
//! frozen clock. Nothing touches the real systemd user manager or `$HOME`.

use serde_json::{Value, json};
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const NOW: &str = "2031-05-14T09:30:00+05:00";
const DATE: &str = "2031-05-14";
const WAIT: Duration = Duration::from_secs(10);

struct Daemon {
    child: Child,
    root: PathBuf,
    runtime: PathBuf,
    config: PathBuf,
    data: PathBuf,
    socket: PathBuf,
    exited: bool,
}

impl Daemon {
    fn start() -> Self {
        let mut daemon = Self::prepare();
        daemon.spawn();
        daemon.wait_for_socket();
        daemon
    }

    fn prepare() -> Self {
        let root = std::env::temp_dir().join(format!(
            "salah-daemon-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let runtime = root.join("runtime");
        fs::create_dir_all(&runtime).unwrap();
        let config = root.join("config.json");
        salah_core::config::save_to(
            &salah_core::config::Config {
                method: "mwl".into(),
                ..salah_core::config::Config::default()
            },
            &config,
        )
        .unwrap();
        let data = root.join("data").join("salah.db");
        let socket = runtime.join("salah").join("salah.sock");
        Self {
            child: Command::new("true").spawn().unwrap(),
            root,
            runtime,
            config,
            data,
            socket,
            exited: false,
        }
    }

    fn spawn(&mut self) {
        let child = Command::new(env!("CARGO_BIN_EXE_salah"))
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("XDG_CONFIG_HOME", self.root.join("xdg-config"))
            .env("XDG_DATA_HOME", self.root.join("xdg-data"))
            .env("XDG_CACHE_HOME", self.root.join("xdg-cache"))
            .env("SALAH_CONFIG", &self.config)
            .env("SALAH_DATA", &self.data)
            .env("SALAH_NOW", NOW)
            .arg("serve")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        self.child = child;
    }

    fn wait_for_socket(&mut self) {
        let deadline = Instant::now() + WAIT;
        while !self.socket.exists() {
            if Instant::now() >= deadline {
                panic!(
                    "daemon did not create {}; status: {:?}",
                    self.socket.display(),
                    self.child.try_wait()
                );
            }
            thread::sleep(Duration::from_millis(10));
        }
        // A file may exist before bind completes; retry connect until it works.
        let _ = self.client();
    }

    fn base_command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_salah"));
        command
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("XDG_CONFIG_HOME", self.root.join("xdg-config"))
            .env("XDG_DATA_HOME", self.root.join("xdg-data"))
            .env("XDG_CACHE_HOME", self.root.join("xdg-cache"))
            .env("SALAH_CONFIG", &self.config)
            .env("SALAH_DATA", &self.data)
            .env("SALAH_NOW", NOW);
        command
    }

    fn cli(&self, args: &[&str]) -> std::process::Output {
        self.base_command().args(args).output().unwrap()
    }

    fn client(&self) -> Client {
        Client::connect(&self.socket)
    }

    fn terminate(&mut self) -> ExitStatus {
        if self.exited {
            return self.child.wait().unwrap();
        }
        let pid = self.child.id().to_string();
        let _ = Command::new("kill").args(["-TERM", &pid]).status().unwrap();
        let status =
            wait_timeout(&mut self.child, WAIT).expect("daemon did not exit after SIGTERM");
        self.exited = true;
        status
    }

    fn wait_until_socket_removed(&self) {
        let deadline = Instant::now() + WAIT;
        while self.socket.exists() {
            assert!(
                Instant::now() < deadline,
                "socket {} was not removed on shutdown",
                self.socket.display()
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if !self.exited {
            let _ = self.terminate();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn wait_timeout(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

struct Client {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    pending: VecDeque<Value>,
    next_id: u64,
}

impl Client {
    fn connect(path: &Path) -> Self {
        let deadline = Instant::now() + WAIT;
        let stream = loop {
            match UnixStream::connect(path) {
                Ok(stream) => break stream,
                Err(error) => {
                    if Instant::now() >= deadline {
                        panic!("connect {}: {error}", path.display());
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        };
        stream
            .set_read_timeout(Some(WAIT))
            .expect("set read timeout");
        stream
            .set_write_timeout(Some(WAIT))
            .expect("set write timeout");
        let reader = BufReader::new(stream.try_clone().unwrap());
        Self {
            stream,
            reader,
            pending: VecDeque::new(),
            next_id: 1,
        }
    }

    fn send(&mut self, value: &Value) {
        let mut line = serde_json::to_string(value).unwrap();
        line.push('\n');
        self.stream.write_all(line.as_bytes()).unwrap();
        self.stream.flush().unwrap();
    }

    fn recv(&mut self) -> Option<Value> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => Some(
                serde_json::from_str(&line)
                    .unwrap_or_else(|error| panic!("daemon wrote non-JSON {line:?}: {error}")),
            ),
            Err(error) => panic!("read from daemon: {error}"),
        }
    }

    fn recv_value(&mut self) -> Value {
        if let Some(value) = self.pending.pop_front() {
            return value;
        }
        self.recv().expect("daemon closed the connection")
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }));
        loop {
            let value = self.recv_value();
            if value.get("id").and_then(Value::as_u64) == Some(id) {
                return value;
            }
            assert!(
                value.get("method").is_some(),
                "unexpected line while waiting for id {id}: {value}"
            );
            self.pending.push_back(value);
        }
    }

    fn hello(&mut self) -> Value {
        self.request(
            "hello",
            json!({"protocol":[1],"client":"daemon-test/1","actor":"test"}),
        )
    }

    fn good(&mut self, method: &str, params: Value) -> Value {
        let response = self.request(method, params);
        assert!(
            response.get("error").is_none(),
            "{method} failed: {}",
            response["error"]
        );
        response["result"].clone()
    }

    fn bad(&mut self, method: &str, params: Value) -> i64 {
        let response = self.request(method, params);
        assert!(
            response.get("result").is_none(),
            "{method} unexpectedly succeeded: {}",
            response["result"]
        );
        response["error"]["code"].as_i64().unwrap()
    }

    fn wait_changed_after(&mut self, revision: i64) -> Value {
        let deadline = Instant::now() + WAIT;
        loop {
            assert!(
                Instant::now() < deadline,
                "no changed notification above revision {revision}"
            );
            let value = self.recv_value();
            if value.get("method").and_then(Value::as_str) == Some("changed") {
                let current = value["params"]["revision"].as_i64().unwrap();
                if current >= revision {
                    return value;
                }
            }
        }
    }
}

fn method_success_and_error(client: &mut Client) {
    // hello is already done; a second hello is valid, a newer protocol is not.
    let hello = client.good("hello", json!({"protocol":[1],"client":"again"}));
    assert_eq!(hello["protocol"], 1);
    assert!(
        hello["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "events.v1")
    );
    assert_eq!(
        client.bad("hello", json!({"protocol":[2],"client":"old"})),
        -32602
    );

    assert!(client.good("status.get", json!({}))["day"].is_string());
    assert_eq!(client.bad("status.get", json!([])), -32602);

    assert_eq!(client.good("day.get", json!({"date": DATE}))["day"], DATE);
    assert_eq!(client.bad("day.get", json!({"date":"not-a-date"})), -32602);

    assert!(client.good("items.list", json!({})).is_array());
    assert_eq!(client.bad("items.list", json!([])), -32602);

    assert_eq!(client.good("stats.get", json!({"days":7}))["days"], 7);
    assert_eq!(
        client.bad("stats.get", json!({"from":"2031-05-01"})),
        -32001
    );

    assert_eq!(
        client
            .good("history.get", json!({}))
            .as_array()
            .unwrap()
            .len(),
        7
    );
    assert_eq!(
        client.bad("history.get", json!({"to":"not-a-date"})),
        -32602
    );

    assert_eq!(
        client.good("item.parse", json!({"text":"read 20m after isha daily"}))["block"],
        "isha"
    );
    assert_eq!(client.bad("item.parse", json!({})), -32602);

    let added = client.good(
        "item.add",
        json!({"op_id":"s4-add","title":"All methods","block":"fajr","measure":"minutes","target":30}),
    );
    let id = added["changed"]["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        client.bad("item.add", json!({"op_id":"s4-add-bad"})),
        -32001
    );

    let extra = client.good(
        "item.add",
        json!({"op_id":"s4-extra","title":"Extra","block":"fajr"}),
    );
    let extra_id = extra["changed"]["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let edited = client.good(
        "item.edit",
        json!({"op_id":"s4-edit","id":id,"title":"Renamed"}),
    );
    assert_eq!(edited["changed"]["items"][0]["title"], "Renamed");
    assert_eq!(
        client.bad(
            "item.edit",
            json!({"op_id":"s4-edit-bad","id":id,"expected_version":999,"title":"Stale"})
        ),
        -32004
    );

    assert!(
        client.good(
            "item.move",
            json!({"op_id":"s4-move","id":extra_id,"block":"fajr","after_id":id})
        )["changed"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == extra_id)
    );
    assert_eq!(
        client.bad(
            "item.move",
            json!({"op_id":"s4-move-bad","id":extra_id,"block":"fajr","after_id":id,"before_id":id})
        ),
        -32001
    );

    assert_eq!(
        client.good("item.remove", json!({"op_id":"s4-remove","id":extra_id}))["changed"]["deleted"],
        json!([extra_id])
    );
    assert_eq!(
        client.bad(
            "item.remove",
            json!({"op_id":"s4-remove-bad","id":"missing"})
        ),
        -32002
    );

    assert_eq!(
        client.good(
            "entry.done",
            json!({"op_id":"s4-done","item_id":id,"date":DATE})
        )["changed"]["items"][0]["state"],
        "done"
    );
    assert_eq!(
        client.bad(
            "entry.done",
            json!({"op_id":"s4-done-bad","item_id":"missing"})
        ),
        -32002
    );

    assert_eq!(
        client.good(
            "entry.undo",
            json!({"op_id":"s4-undo","item_id":id,"date":DATE})
        )["undone"],
        1
    );
    assert_eq!(
        client.bad(
            "entry.undo",
            json!({"op_id":"s4-undo-bad","item_id":"missing"})
        ),
        -32002
    );

    assert_eq!(
        client.good(
            "entry.log",
            json!({"op_id":"s4-log","item_id":id,"kind":"minutes","value":5})
        )["changed"]["items"][0]["minutes"],
        5
    );
    assert_eq!(
        client.bad(
            "entry.log",
            json!({"op_id":"s4-log-bad","item_id":id,"value":0})
        ),
        -32001
    );

    assert!(
        client.good("timer.start", json!({"op_id":"s4-start","item_id":id}))["changed"]["items"][0]
            ["timer_started_at"]
            .is_string()
    );
    assert_eq!(
        client.bad(
            "timer.start",
            json!({"op_id":"s4-start-bad","item_id":"missing"})
        ),
        -32002
    );

    assert_eq!(
        client.good("timer.stop", json!({"op_id":"s4-stop","item_id":id}))["changed"]["entries"][0]
            ["kind"],
        "timer"
    );
    assert_eq!(
        client.bad(
            "timer.stop",
            json!({"op_id":"s4-stop-bad","item_id":"missing"})
        ),
        -32002
    );

    assert_eq!(
        client.good(
            "prayer.mark",
            json!({"op_id":"s4-mark","prayer":"dhuhr","date":DATE})
        )["changed"]["marks"][0]["prayer"],
        "dhuhr"
    );
    assert_eq!(
        client.bad(
            "prayer.mark",
            json!({"op_id":"s4-mark-bad","prayer":"sleep"})
        ),
        -32001
    );

    assert_eq!(
        client.good(
            "prayer.unmark",
            json!({"op_id":"s4-unmark","prayer":"dhuhr","date":DATE})
        )["changed"]["marks"][0]["prayer"],
        "dhuhr"
    );
    assert_eq!(
        client.bad(
            "prayer.unmark",
            json!({"op_id":"s4-unmark-bad","prayer":"sleep"})
        ),
        -32001
    );

    assert_eq!(client.good("config.get", json!({}))["method"], "mwl");
    assert_eq!(client.bad("config.get", json!([])), -32602);

    assert_eq!(
        client.good(
            "config.set",
            json!({"op_id":"s4-config","sleep_time":"22:15"})
        )["sleep_time"],
        "22:15"
    );
    assert_eq!(
        client.bad("config.set", json!({"op_id":"s4-config-bad"})),
        -32001
    );

    let exported = client.good("export", json!({}));
    assert_eq!(exported["format"], "salah-export");
    assert_eq!(client.bad("export", json!([])), -32602);

    assert!(
        client.good("import", json!({"op_id":"s4-import","data":exported}))["summary"]["unchanged"]
            .is_number()
    );
    assert_eq!(
        client.bad(
            "import",
            json!({"op_id":"s4-import-bad","data":{"format":"not-salah"}})
        ),
        -32602
    );

    let subscribed = client.good(
        "subscribe",
        json!({"topics":["changes"],"after_revision":0}),
    );
    assert!(subscribed["revision"].as_i64().unwrap() > 0);
    assert_eq!(
        client.bad("subscribe", json!({"topics":["unknown"]})),
        -32001
    );
}

#[test]
fn socket_permissions_and_handshake() {
    let daemon = Daemon::start();
    let dir = daemon.socket.parent().unwrap();
    assert_eq!(
        fs::metadata(dir).unwrap().permissions().mode() & 0o777,
        0o700,
        "runtime directory must be private"
    );
    assert_eq!(
        fs::metadata(&daemon.socket).unwrap().permissions().mode() & 0o777,
        0o600,
        "socket must be user-only"
    );

    let mut client = daemon.client();
    assert_eq!(client.bad("status.get", json!({})), -32600);
    let hello = client.hello();
    assert_eq!(hello["result"]["protocol"], 1);
    assert!(
        hello["result"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "events.v1")
    );
    assert!(client.good("status.get", json!({}))["day"].is_string());
}

#[test]
fn every_protocol_method_has_a_success_and_an_error_over_the_socket() {
    let daemon = Daemon::start();
    let mut client = daemon.client();
    client.hello();
    method_success_and_error(&mut client);
}

#[test]
fn subscribers_receive_changed_for_socket_and_plain_cli_writes() {
    let daemon = Daemon::start();
    let mut subscriber = daemon.client();
    subscriber.hello();
    let subscribed = subscriber.good("subscribe", json!({"topics":["changes"]}));
    let base_revision = subscribed["revision"].as_i64().unwrap();

    let mut writer = daemon.client();
    writer.hello();
    let added = writer.good(
        "item.add",
        json!({"op_id":"s4-sub-socket","title":"From socket","block":"asr","date":DATE}),
    );
    let revision = added["revision"].as_i64().unwrap();
    assert!(revision > base_revision);
    let notification = subscriber.wait_changed_after(revision);
    assert_eq!(notification["params"]["revision"], revision);
    assert!(
        notification["params"]["scopes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|scope| scope == "items")
    );

    let output = daemon.cli(&[
        "item", "add", "From CLI", "--block", "asr", "--date", DATE, "--json",
    ]);
    assert!(
        output.status.success(),
        "CLI add failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let notification = subscriber.wait_changed_after(revision + 1);
    assert!(notification["params"]["revision"].as_i64().unwrap() > revision);
}

#[test]
fn concurrent_writers_keep_revisions_gapless() {
    const WRITERS: usize = 4;
    const ADDS: usize = 5;
    let daemon = Arc::new(Daemon::start());
    let barrier = Arc::new(Barrier::new(WRITERS));
    let mut handles = Vec::new();
    for writer in 0..WRITERS {
        let daemon = Arc::clone(&daemon);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            let mut client = daemon.client();
            client.hello();
            barrier.wait();
            let mut revisions = Vec::new();
            for add in 0..ADDS {
                let result = client.good(
                    "item.add",
                    json!({
                        "op_id": format!("s4-concurrent-{writer}-{add}"),
                        "title": format!("writer {writer} add {add}"),
                        "block": "asr",
                        "date": DATE,
                    }),
                );
                revisions.push(result["revision"].as_i64().unwrap());
            }
            revisions
        }));
    }
    let mut revisions = Vec::new();
    for handle in handles {
        revisions.extend(handle.join().unwrap());
    }
    revisions.sort_unstable();
    assert_eq!(
        revisions,
        (1..=(WRITERS * ADDS) as i64).collect::<Vec<_>>(),
        "one serialised writer must allocate every revision exactly once"
    );

    let mut reader = daemon.client();
    reader.hello();
    assert_eq!(
        reader
            .good("items.list", json!({}))
            .as_array()
            .unwrap()
            .len(),
        WRITERS * ADDS
    );
}

#[test]
fn a_second_daemon_refuses_to_steal_the_live_socket() {
    let daemon = Daemon::start();
    let mut second = daemon
        .base_command()
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let status = wait_timeout(&mut second, WAIT);
    let status = match status {
        Some(status) => status,
        None => {
            let _ = second.kill();
            let _ = second.wait();
            panic!("second daemon kept running instead of refusing the live socket");
        }
    };
    assert!(!status.success());
    let bytes = second
        .stderr
        .take()
        .map(|mut stderr| {
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut stderr, &mut bytes).unwrap();
            bytes
        })
        .unwrap_or_default();
    let error = String::from_utf8_lossy(&bytes);
    assert!(
        error.contains("already listening"),
        "unexpected second-daemon error: {error}"
    );

    let mut client = daemon.client();
    client.hello();
    assert!(client.good("status.get", json!({}))["day"].is_string());
}

#[test]
fn a_stale_socket_file_is_replaced_and_sigterm_removes_it() {
    let mut daemon = Daemon::prepare();
    let dir = daemon.socket.parent().unwrap();
    fs::create_dir_all(dir).unwrap();
    // Bind and drop a listener to leave a real socket file without a server.
    drop(std::os::unix::net::UnixListener::bind(&daemon.socket).unwrap());
    assert!(daemon.socket.exists());

    daemon.spawn();
    daemon.wait_for_socket();
    let mut client = daemon.client();
    client.hello();
    assert!(client.good("status.get", json!({}))["day"].is_string());

    let status = daemon.terminate();
    assert!(
        status.success(),
        "SIGTERM shutdown should succeed: {status}"
    );
    daemon.wait_until_socket_removed();
}

#[test]
fn rpc_socket_bridges_stdio_to_the_daemon() {
    let daemon = Daemon::start();
    let mut child = daemon
        .base_command()
        .args(["rpc", "--socket"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":1,"method":"hello","params":{{"protocol":[1],"client":"bridge"}}}}"#
        )
        .unwrap();
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "bridge failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let line = String::from_utf8_lossy(&output.stdout);
    let response: Value = serde_json::from_str(line.lines().next().unwrap()).unwrap();
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["protocol"], 1);
}
