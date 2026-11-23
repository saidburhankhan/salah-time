//! End-to-end tests for `salah serve`: the protocol-v1 daemon.
//!
//! Every test uses a fresh temporary runtime/config/data directory and a
//! frozen clock. Nothing touches the real systemd user manager or `$HOME`.

use serde_json::{Value, json};
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
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

    fn wait_method(&mut self, method: &str) -> Value {
        let deadline = Instant::now() + WAIT;
        loop {
            assert!(
                Instant::now() < deadline,
                "no {method} notification arrived"
            );
            let value = self.recv_value();
            if value.get("method").and_then(Value::as_str) == Some(method) {
                return value;
            }
        }
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

#[path = "daemon/methods.rs"]
mod methods;
#[path = "daemon/transport.rs"]
mod transport;
