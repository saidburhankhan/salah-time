//! `salah serve`: the long-running single-writer daemon.
//!
//! The transport is protocol v1's NDJSON JSON-RPC over a user-only Unix
//! socket. Each connection gets a reader thread (this function) and a writer
//! thread that drains an outbound queue, so `changed` notifications can be
//! pushed while the reader waits for the next request. Mutations are sent to
//! one writer thread; reads use each connection's own SQLite connection.
//!
//! A watcher thread polls `PRAGMA data_version` so writes made by a plain
//! `salah` CLI process, which never talks to the daemon, still produce
//! `changed` notifications.

use crate::local::XdgConfig;
use chrono::{DateTime, NaiveDate, Utc};
use salah_core::status;
use salah_domain::day::BoundaryId;
use salah_domain::runtime::{Clock, FixedClock, SystemClock, UuidV4};
use salah_protocol::{
    ChangeScope, ChangedNotification, ClockBoundaryNext, ClockBoundaryNotification,
    ConfigRepository, Context, Dispatcher, Request, Response, RpcError,
};
use salah_store::{Store, default_path};
use serde::Serialize;
use serde_json::Value;
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::os::fd::FromRawFd;
use std::os::raw::c_int;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

const SOCKET_DIR: &str = "salah";
const SOCKET_NAME: &str = "salah.sock";
const WATCH_INTERVAL: Duration = Duration::from_millis(250);
const ACCEPT_TICK: Duration = Duration::from_millis(50);
const MAX_LINE_BYTES: usize = salah_protocol::MAX_LINE_BYTES;
const SIGINT: c_int = 2;
const SIGTERM: c_int = 15;

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn signal(signum: c_int, handler: usize) -> usize;
}

extern "C" fn on_signal(_: c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

fn install_signals() {
    // The handler only performs an atomic store, which is async-signal-safe.
    unsafe {
        signal(SIGINT, on_signal as *const () as usize);
        signal(SIGTERM, on_signal as *const () as usize);
    }
}

/// The canonical socket path. There is deliberately no fallback to `/tmp` or
/// TCP: without a private runtime directory the daemon refuses to start.
pub fn socket_path() -> Result<PathBuf, String> {
    let runtime = env::var_os("XDG_RUNTIME_DIR")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "XDG_RUNTIME_DIR is not set; salah serve needs a private runtime directory".to_string()
        })?;
    Ok(PathBuf::from(runtime).join(SOCKET_DIR).join(SOCKET_NAME))
}

#[derive(Clone, Copy)]
enum DaemonClock {
    System(SystemClock),
    Fixed(FixedClock),
}

impl Clock for DaemonClock {
    fn now(&self) -> DateTime<Utc> {
        match self {
            Self::System(clock) => clock.now(),
            Self::Fixed(clock) => clock.now(),
        }
    }
}

fn daemon_clock() -> Result<DaemonClock, String> {
    match env::var("SALAH_NOW") {
        Ok(value) => {
            let now = DateTime::parse_from_rfc3339(&value)
                .map_err(|_| format!("invalid SALAH_NOW {value:?}; use RFC 3339"))?
                .with_timezone(&Utc);
            Ok(DaemonClock::Fixed(FixedClock::new(now)))
        }
        Err(env::VarError::NotPresent) => Ok(DaemonClock::System(SystemClock)),
        Err(error) => Err(format!("invalid SALAH_NOW: {error}")),
    }
}

struct Shared {
    subscribers: Mutex<Vec<(u64, mpsc::Sender<Vec<u8>>)>>,
    next_subscriber: AtomicU64,
    last_activity: Mutex<Instant>,
    boundary: Mutex<Option<ClockBoundaryNotification>>,
}

impl Shared {
    fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
            next_subscriber: AtomicU64::new(1),
            last_activity: Mutex::new(Instant::now()),
            boundary: Mutex::new(None),
        }
    }

    fn set_boundary(&self, boundary: ClockBoundaryNotification) {
        *self.boundary.lock().unwrap() = Some(boundary);
    }

    fn boundary(&self) -> Option<ClockBoundaryNotification> {
        self.boundary.lock().unwrap().clone()
    }

    fn touch(&self) {
        *self.last_activity.lock().unwrap() = Instant::now();
    }

    fn idle_for(&self) -> Duration {
        self.last_activity.lock().unwrap().elapsed()
    }

    fn subscribe(&self, id: u64, sender: mpsc::Sender<Vec<u8>>) {
        self.subscribers.lock().unwrap().push((id, sender));
    }

    fn unsubscribe(&self, id: u64) {
        self.subscribers
            .lock()
            .unwrap()
            .retain(|(key, _)| *key != id);
    }

    fn broadcast(&self, line: &str) {
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\n');
        let mut subscribers = self.subscribers.lock().unwrap();
        subscribers.retain(|(_, sender)| sender.send(bytes.clone()).is_ok());
    }
}

struct WriteJob {
    request: Request,
    reply: mpsc::Sender<Response>,
}

/// Runs the daemon until SIGTERM/SIGINT, or until an idle socket-activated
/// instance has had no connections for `idle_minutes`.
pub fn serve(idle_minutes: u64) -> Result<(), String> {
    install_signals();
    let clock = daemon_clock()?;
    let data = default_path();
    let config = XdgConfig {
        path: salah_core::config::path(),
    };

    let (listener, activated, owned_socket) = open_listener()?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("listen: {error}"))?;

    let shared = Arc::new(Shared::new());
    let active = Arc::new(AtomicUsize::new(0));

    let (write_tx, write_rx) = mpsc::channel::<WriteJob>();
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), String>>(1);
    let writer_data = data.clone();
    let writer_clock = clock;
    let writer_config_path = config.path.clone();
    thread::spawn(move || {
        writer_loop(
            writer_data,
            writer_config_path,
            writer_clock,
            write_rx,
            ready_tx,
        )
    });
    match ready_rx
        .recv()
        .map_err(|_| "writer thread stopped before opening the store".to_string())?
    {
        Ok(()) => {}
        Err(error) => return Err(error),
    }

    let watcher_shared = Arc::clone(&shared);
    let watcher_data = data.clone();
    thread::spawn(move || watcher_loop(watcher_data, watcher_shared));

    let boundary_shared = Arc::clone(&shared);
    let boundary_config = config.path.clone();
    thread::spawn(move || boundary_loop(boundary_config, clock, boundary_shared));

    let idle = Duration::from_secs(idle_minutes.saturating_mul(60));
    let mut last_transition = Instant::now();
    let mut connections = 0usize;

    while !SHUTDOWN.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                shared.touch();
                last_transition = Instant::now();
                connections += 1;
                active.fetch_add(1, Ordering::SeqCst);
                let shared = Arc::clone(&shared);
                let active = Arc::clone(&active);
                let write_tx = write_tx.clone();
                let data = data.clone();
                let config_path = config.path.clone();
                let connection_clock = clock;
                thread::spawn(move || {
                    handle_connection(
                        stream,
                        shared,
                        active,
                        write_tx,
                        data,
                        config_path,
                        connection_clock,
                    );
                });
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                eprintln!("salah: accept on {}: {error}", socket_path()?.display());
                thread::sleep(ACCEPT_TICK);
            }
        }

        let now_active = active.load(Ordering::SeqCst);
        if now_active != connections {
            connections = now_active;
            last_transition = Instant::now();
        }
        if activated
            && idle_minutes > 0
            && now_active == 0
            && last_transition.elapsed() >= idle
            && shared.idle_for() >= idle
        {
            break;
        }
        thread::sleep(ACCEPT_TICK);
    }

    if let Some(path) = owned_socket {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

/// Returns the listener, whether it came from systemd activation, and the path
/// this process owns (and must remove on shutdown).
fn open_listener() -> Result<(UnixListener, bool, Option<PathBuf>), String> {
    if let Some(listener) = activated_listener()? {
        return Ok((listener, true, None));
    }
    let path = socket_path()?;
    let dir = path
        .parent()
        .ok_or_else(|| format!("invalid socket path {}", path.display()))?;
    fs::create_dir_all(dir).map_err(|error| format!("create {}: {error}", dir.display()))?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("chmod {}: {error}", dir.display()))?;

    if path.exists() {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("stat {}: {error}", path.display()))?;
        if !metadata.file_type().is_socket() {
            return Err(format!(
                "{} exists and is not a socket; refusing to replace it",
                path.display()
            ));
        }
        match UnixStream::connect(&path) {
            Ok(_) => {
                return Err(format!(
                    "another salah serve is already listening on {}",
                    path.display()
                ));
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                ) =>
            {
                fs::remove_file(&path)
                    .map_err(|error| format!("remove stale {}: {error}", path.display()))?;
            }
            Err(error) => {
                return Err(format!(
                    "cannot check existing socket {}: {error}; refusing to remove it",
                    path.display()
                ));
            }
        }
    }

    let listener =
        UnixListener::bind(&path).map_err(|error| format!("bind {}: {error}", path.display()))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("chmod {}: {error}", path.display()))?;
    Ok((listener, false, Some(path)))
}

/// systemd's sd_listen_fds contract: LISTEN_PID must name this process and
/// LISTEN_FDS must count at least our one listening socket at fd 3.
fn activated_listener() -> Result<Option<UnixListener>, String> {
    let Some(fds) = env::var_os("LISTEN_FDS") else {
        return Ok(None);
    };
    let fds: usize = fds
        .to_string_lossy()
        .parse()
        .map_err(|_| "invalid LISTEN_FDS".to_string())?;
    let pid: u32 = env::var("LISTEN_PID")
        .unwrap_or_default()
        .parse()
        .map_err(|_| "LISTEN_FDS is set but LISTEN_PID is not a pid".to_string())?;
    if pid != process::id() {
        return Err(format!(
            "LISTEN_PID {pid} does not match this process {}; refusing socket activation",
            process::id()
        ));
    }
    if fds == 0 {
        return Ok(None);
    }
    if fds != 1 {
        return Err(format!(
            "salah expects exactly one activated socket, LISTEN_FDS is {fds}"
        ));
    }
    // The daemon is the systemd service process, so fd 3 is ours. Remove the
    // activation variables so child processes do not inherit them.
    unsafe {
        env::remove_var("LISTEN_FDS");
        env::remove_var("LISTEN_PID");
        Ok(Some(UnixListener::from_raw_fd(3)))
    }
}

fn writer_loop(
    data: PathBuf,
    config_path: PathBuf,
    clock: DaemonClock,
    jobs: mpsc::Receiver<WriteJob>,
    ready: mpsc::SyncSender<Result<(), String>>,
) {
    let mut store = match Store::open(&data) {
        Ok(store) => {
            let _ = ready.send(Ok(()));
            store
        }
        Err(error) => {
            let _ = ready.send(Err(error.to_string()));
            return;
        }
    };
    let config = XdgConfig { path: config_path };
    let ids = UuidV4;
    let mut dispatcher = Dispatcher::new();
    dispatcher.accept_hello();
    while let Ok(job) = jobs.recv() {
        let mut context = Context {
            store: &mut store,
            config: &config,
            clock: &clock,
            ids: &ids,
        };
        let response = dispatcher.dispatch(&job.request, &mut context);
        let _ = job.reply.send(response);
    }
}

fn watcher_loop(data: PathBuf, shared: Arc<Shared>) {
    let store = match Store::open(&data) {
        Ok(store) => store,
        Err(error) => {
            eprintln!(
                "salah: change watcher cannot open {}: {error}",
                data.display()
            );
            return;
        }
    };
    let mut last_data_version = store.data_version().unwrap_or(0);
    let mut last_seq = store.last_change_seq().unwrap_or(0);
    while !SHUTDOWN.load(Ordering::SeqCst) {
        thread::sleep(WATCH_INTERVAL);
        let data_version = match store.data_version() {
            Ok(version) => version,
            Err(error) => {
                eprintln!("salah: data_version: {error}");
                continue;
            }
        };
        if data_version == last_data_version {
            continue;
        }
        last_data_version = data_version;
        loop {
            let changes = match store.changes_since(last_seq, 10_000) {
                Ok(changes) => changes,
                Err(error) => {
                    eprintln!("salah: read changes: {error}");
                    break;
                }
            };
            if changes.is_empty() {
                break;
            }
            let mut revision = None;
            let mut scopes = Vec::new();
            let mut op_id = None;
            for change in &changes {
                last_seq = change.seq;
                if revision != Some(change.revision) {
                    if let Some(revision) = revision {
                        emit_changed(&shared, revision, &scopes, &op_id);
                    }
                    revision = Some(change.revision);
                    scopes.clear();
                    op_id = None;
                }
                if let Some(scope) = ChangeScope::of_entity(&change.entity)
                    && !scopes.contains(&scope)
                {
                    scopes.push(scope);
                }
                if op_id.is_none() {
                    op_id.clone_from(&change.op_id);
                }
            }
            if let Some(revision) = revision {
                emit_changed(&shared, revision, &scopes, &op_id);
            }
            if changes.len() < 10_000 {
                break;
            }
        }
    }
}

/// The current block and next boundary as the daemon sees them.
fn clock_boundary(
    clock: &DaemonClock,
    config: &XdgConfig,
) -> Result<ClockBoundaryNotification, String> {
    let settings = config.load().map_err(|error| error.to_string())?;
    let now = clock.now();
    let report = status::build(&settings, now).map_err(|error| error.to_string())?;
    let day = NaiveDate::parse_from_str(&report.day, "%Y-%m-%d")
        .map_err(|error| format!("status day: {error}"))?;
    let next = BoundaryId::parse(report.next.id)
        .ok_or_else(|| format!("unknown next boundary {:?}", report.next.id))?;
    Ok(ClockBoundaryNotification {
        day,
        block: Some(report.current_block.block.start),
        now,
        next: ClockBoundaryNext {
            id: next,
            name: report.next.name.to_string(),
            time: report.next.time,
            at: report.next.at,
            remaining_seconds: report.next.remaining_seconds,
        },
    })
}

/// Polls the schedule and broadcasts `clock.boundary` when the current block or
/// next boundary identity changes. `remaining_seconds` inside a notification is
/// only a snapshot; clients count down from `next.at`.
fn boundary_loop(config_path: PathBuf, clock: DaemonClock, shared: Arc<Shared>) {
    let config = XdgConfig { path: config_path };
    let mut last = shared.boundary();
    while !SHUTDOWN.load(Ordering::SeqCst) {
        if let Ok(current) = clock_boundary(&clock, &config) {
            let changed = last
                .as_ref()
                .is_none_or(|previous| boundary_key(previous) != boundary_key(&current));
            if changed {
                shared.set_boundary(current.clone());
                if let Ok(line) = serde_json::to_string(&current.clone().envelope()) {
                    shared.broadcast(&line);
                }
                last = Some(current);
            }
        }
        thread::sleep(Duration::from_secs(1));
    }
}

fn boundary_key(
    boundary: &ClockBoundaryNotification,
) -> (NaiveDate, Option<BoundaryId>, BoundaryId, DateTime<Utc>) {
    (
        boundary.day,
        boundary.block,
        boundary.next.id,
        boundary.next.at,
    )
}

fn emit_changed(shared: &Shared, revision: i64, scopes: &[ChangeScope], op_id: &Option<String>) {
    let notification = ChangedNotification {
        revision,
        days: Vec::new(),
        scopes: scopes.to_vec(),
        op_id: op_id.clone(),
        actor: "daemon".into(),
    };
    if let Ok(line) = serde_json::to_string(&notification.envelope()) {
        shared.broadcast(&line);
    }
}

fn handle_connection(
    stream: UnixStream,
    shared: Arc<Shared>,
    active: Arc<AtomicUsize>,
    write_tx: mpsc::Sender<WriteJob>,
    data: PathBuf,
    config_path: PathBuf,
    clock: DaemonClock,
) {
    let writer_stream = match stream.try_clone() {
        Ok(stream) => stream,
        Err(_) => {
            active.fetch_sub(1, Ordering::SeqCst);
            shared.touch();
            return;
        }
    };
    let (out_tx, out_rx) = mpsc::channel::<Vec<u8>>();
    let writer_handle = thread::spawn(move || {
        let mut output = BufWriter::new(writer_stream);
        for bytes in out_rx {
            if output.write_all(&bytes).is_err() || output.flush().is_err() {
                break;
            }
        }
    });

    let mut reader = BufReader::new(stream);
    let mut store = match Store::open(&data) {
        Ok(store) => store,
        Err(error) => {
            send_response(
                &out_tx,
                &Response::err(
                    Value::Null,
                    RpcError::internal(format!("open store for connection: {error}")),
                ),
            );
            drop(out_tx);
            let _ = writer_handle.join();
            active.fetch_sub(1, Ordering::SeqCst);
            shared.touch();
            return;
        }
    };
    let config = XdgConfig { path: config_path };
    let ids = UuidV4;
    let mut dispatcher = Dispatcher::new();
    let subscriber_id = shared.next_subscriber.fetch_add(1, Ordering::SeqCst);
    let mut subscribed = false;

    loop {
        let (raw, oversize) = match read_line_bounded(&mut reader, MAX_LINE_BYTES) {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(_) => break,
        };
        if oversize {
            let response = Response::err(
                Value::Null,
                RpcError::parse(format!(
                    "request line exceeds the {MAX_LINE_BYTES}-byte limit"
                )),
            );
            if !send_response(&out_tx, &response) {
                break;
            }
            continue;
        }
        let line = match String::from_utf8(raw) {
            Ok(line) => line,
            Err(_) => {
                let response = Response::err(
                    Value::Null,
                    RpcError::parse("request line is not valid UTF-8"),
                );
                if !send_response(&out_tx, &response) {
                    break;
                }
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                let response = Response::err(
                    Value::Null,
                    RpcError::parse(format!("invalid JSON: {error}")),
                );
                if !send_response(&out_tx, &response) {
                    break;
                }
                continue;
            }
        };
        let id = value.get("id").cloned().unwrap_or(Value::Null);
        let request = match Request::parse(value) {
            Ok(request) => request,
            Err(error) => {
                if !send_response(&out_tx, &Response::err(id, error)) {
                    break;
                }
                continue;
            }
        };

        // `subscribe` is connection state: dispatch it locally to validate the
        // parameters, then attach this connection's outbound queue.
        if request.method == "subscribe" {
            let requested: Vec<String> = request
                .params
                .get("topics")
                .and_then(Value::as_array)
                .map(|topics| {
                    topics
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            if !dispatcher.hello_done() {
                let response = Response::err(
                    request.id.clone(),
                    RpcError::invalid_request("hello must be the first request on a connection"),
                );
                if !send_response(&out_tx, &response) {
                    break;
                }
                continue;
            }
            let response = {
                let mut context = Context {
                    store: &mut store,
                    config: &config,
                    clock: &clock,
                    ids: &ids,
                };
                dispatcher.dispatch(&request, &mut context)
            };
            let ok = response.error.is_none();
            if !send_response(&out_tx, &response) {
                break;
            }
            if ok && !subscribed {
                shared.subscribe(subscriber_id, out_tx.clone());
                subscribed = true;
                let wants_clock = requested.iter().any(|topic| topic == "clock");
                if wants_clock {
                    let notification = shared
                        .boundary()
                        .map(Ok)
                        .unwrap_or_else(|| clock_boundary(&clock, &config));
                    if let Ok(notification) = notification {
                        let _ = send_notification(&out_tx, &notification.envelope());
                    }
                }
            }
            continue;
        }

        let response = if Dispatcher::is_mutation(&request.method) {
            if !dispatcher.hello_done() {
                Response::err(
                    request.id.clone(),
                    RpcError::invalid_request("hello must be the first request on a connection"),
                )
            } else {
                let (reply_tx, reply_rx) = mpsc::channel();
                let job = WriteJob {
                    request,
                    reply: reply_tx,
                };
                if write_tx.send(job).is_err() {
                    Response::err(id, RpcError::internal("the daemon writer thread stopped"))
                } else {
                    match reply_rx.recv_timeout(Duration::from_secs(30)) {
                        Ok(response) => response,
                        Err(_) => Response::err(
                            id,
                            RpcError::internal("the daemon writer did not answer in 30 s"),
                        ),
                    }
                }
            }
        } else {
            let mut context = Context {
                store: &mut store,
                config: &config,
                clock: &clock,
                ids: &ids,
            };
            dispatcher.dispatch(&request, &mut context)
        };

        if !send_response(&out_tx, &response) {
            break;
        }
    }

    if subscribed {
        shared.unsubscribe(subscriber_id);
    }
    drop(out_tx);
    let _ = writer_handle.join();
    active.fetch_sub(1, Ordering::SeqCst);
    shared.touch();
}

fn send_response(sender: &mpsc::Sender<Vec<u8>>, response: &Response) -> bool {
    let Ok(mut line) = response.clone().into_line() else {
        return false;
    };
    line.push('\n');
    sender.send(line.into_bytes()).is_ok()
}

fn send_notification<T: Serialize>(
    sender: &mpsc::Sender<Vec<u8>>,
    notification: &salah_protocol::Notification<T>,
) -> bool {
    let Ok(mut line) = serde_json::to_string(notification) else {
        return false;
    };
    line.push('\n');
    sender.send(line.into_bytes()).is_ok()
}

/// Pure-Rust socket bridge for `salah rpc --socket [PATH]`.
pub fn bridge(socket: Option<&std::path::Path>) -> Result<(), String> {
    let path = match socket {
        Some(path) => path.to_path_buf(),
        None => socket_path()?,
    };
    let mut stream = UnixStream::connect(&path)
        .map_err(|error| format!("connect {}: {error}", path.display()))?;
    let reader_stream = stream
        .try_clone()
        .map_err(|error| format!("clone socket: {error}"))?;
    let input = thread::spawn(move || {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        let mut line = String::new();
        loop {
            line.clear();
            match input.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if stream.write_all(line.as_bytes()).is_err() {
                        break;
                    }
                    let _ = stream.flush();
                }
                Err(_) => break,
            }
        }
        let _ = stream.shutdown(std::net::Shutdown::Write);
    });
    let mut reader = BufReader::new(reader_stream);
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) => break,
            Ok(_) => {
                if output.write_all(&line).is_err() {
                    break;
                }
                let _ = output.flush();
            }
            Err(error) => return Err(format!("read socket: {error}")),
        }
    }
    drop(input);
    Ok(())
}

fn read_line_bounded(
    reader: &mut impl BufRead,
    limit: usize,
) -> io::Result<Option<(Vec<u8>, bool)>> {
    let mut line = Vec::new();
    let mut oversize = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if line.is_empty() && !oversize {
                return Ok(None);
            }
            return Ok(Some((line, oversize)));
        }
        if let Some(index) = available.iter().position(|byte| *byte == b'\n') {
            let chunk = &available[..=index];
            if !oversize {
                if line.len() + chunk.len() > limit {
                    oversize = true;
                    line.clear();
                } else {
                    line.extend_from_slice(chunk);
                }
            }
            reader.consume(index + 1);
            return Ok(Some((line, oversize)));
        }
        let length = available.len();
        if !oversize {
            if line.len() + length > limit {
                oversize = true;
                line.clear();
            } else {
                line.extend_from_slice(available);
            }
        }
        reader.consume(length);
    }
}
