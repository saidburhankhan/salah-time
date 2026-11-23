//! Real-process concurrency checks for the tracking store.
//!
//! Every command runs the actual `salah` binary against one temporary
//! `SALAH_DATA`, so these tests exercise the same SQLite locking a desktop app
//! and several widgets do.

use salah_core::config::Config;
use salah_domain::day::BoundaryId;
use salah_store::Store;
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread::{self, JoinHandle};
use std::time::Duration;

static NEXT: AtomicUsize = AtomicUsize::new(0);
const NOW: &str = "2031-05-14T09:30:00+05:00";
const DATE: &str = "2031-05-14";

struct Fixture {
    root: PathBuf,
    config: PathBuf,
    data: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let fixture = Self::uninitialised();
        // Create the schema once so most tests only race on data writes.
        Store::open(&fixture.data).unwrap();
        fixture
    }

    fn uninitialised() -> Self {
        let root = std::env::temp_dir().join(format!(
            "salah-concurrency-{}-{}",
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
        let data = root.join("planner.db");
        Self { root, config, data }
    }

    fn run(&self, args: &[String]) -> Output {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        Command::new(env!("CARGO_BIN_EXE_salah"))
            .env("SALAH_CONFIG", &self.config)
            .env("SALAH_DATA", &self.data)
            .env("SALAH_NOW", NOW)
            .args(refs)
            .output()
            .unwrap()
    }

    fn json(&self, args: &[String]) -> Value {
        let output = self.run(args);
        assert_success(&output, args);
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "not JSON from {args:?}: {error}\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        })
    }

    fn add(&self, title: &str) -> String {
        let args = strings([
            "item", "add", title, "--block", "asr", "--date", DATE, "--json",
        ]);
        self.json(&args)["changed"]["items"][0]["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn store(&self) -> Store {
        Store::open(&self.data).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn strings<const N: usize>(parts: [&str; N]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_string()).collect()
}

fn assert_success(output: &Output, args: &[String]) {
    assert!(
        output.status.success(),
        "command failed: {args:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn live_positions(store: &Store, block: BoundaryId) -> Vec<i64> {
    let mut positions: Vec<i64> = store
        .items()
        .unwrap()
        .into_iter()
        .filter(|item| item.is_live() && item.block == Some(block))
        .map(|item| item.position)
        .collect();
    positions.sort_unstable();
    positions
}

fn assert_dense(positions: &[i64]) {
    let expected: Vec<i64> = (0..positions.len() as i64).collect();
    assert_eq!(
        positions, expected,
        "positions must be 0..n-1 without gaps or duplicates"
    );
}

/// Readers run until stopped; dropping the guard also stops them, so a failed
/// writer assertion cannot leave them spinning.
struct Readers {
    stop: Arc<AtomicBool>,
    handles: Option<Vec<JoinHandle<()>>>,
}

impl Readers {
    fn spawn(fixture: Arc<Fixture>, count: usize) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let handles = (0..count)
            .map(|_| {
                let fixture = Arc::clone(&fixture);
                let stop = Arc::clone(&stop);
                thread::spawn(move || {
                    let commands = [
                        strings(["item", "list", "--json"]),
                        strings(["day", "--json"]),
                        strings(["stats", "--json"]),
                        strings(["status", "--json"]),
                        strings(["project", "list", "--json"]),
                    ];
                    while !stop.load(Ordering::SeqCst) {
                        for command in &commands {
                            let output = fixture.run(command);
                            assert_success(&output, command);
                            serde_json::from_slice::<Value>(&output.stdout).unwrap_or_else(|e| {
                                panic!("read {command:?} returned invalid JSON: {e}")
                            });
                        }
                        thread::sleep(Duration::from_millis(1));
                    }
                })
            })
            .collect();
        Self {
            stop,
            handles: Some(handles),
        }
    }

    fn finish(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        for handle in self.handles.take().unwrap() {
            handle.join().unwrap();
        }
    }
}

impl Drop for Readers {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handles) = self.handles.take() {
            for handle in handles {
                let _ = handle.join();
            }
        }
    }
}

#[test]
fn concurrent_adds_share_one_dense_position_sequence() {
    const WRITERS: usize = 8;
    const ADDS: usize = 5;
    let fixture = Arc::new(Fixture::new());
    let mut handles = Vec::new();
    for writer in 0..WRITERS {
        let fixture = Arc::clone(&fixture);
        handles.push(thread::spawn(move || {
            for add in 0..ADDS {
                let title = format!("writer {writer} add {add}");
                let args = strings([
                    "item", "add", &title, "--block", "asr", "--date", DATE, "--json",
                ]);
                let output = fixture.run(&args);
                assert_success(&output, &args);
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }

    let store = fixture.store();
    let positions = live_positions(&store, BoundaryId::Asr);
    assert_eq!(positions.len(), WRITERS * ADDS);
    assert_dense(&positions);
}

#[test]
fn concurrent_first_opens_migrate_once_without_failing() {
    const WRITERS: usize = 6;
    let fixture = Arc::new(Fixture::uninitialised());
    let barrier = Arc::new(Barrier::new(WRITERS));
    let mut handles = Vec::new();
    for writer in 0..WRITERS {
        let fixture = Arc::clone(&fixture);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            let title = format!("fresh {writer}");
            let args = strings([
                "item", "add", &title, "--block", "asr", "--date", DATE, "--json",
            ]);
            let output = fixture.run(&args);
            assert_success(&output, &args);
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }

    let store = fixture.store();
    assert_eq!(store.version().unwrap(), salah_store::schema_version());
    assert_eq!(live_positions(&store, BoundaryId::Asr).len(), WRITERS);
    assert!(std::fs::read_dir(&fixture.root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".old-")
    }));
}

#[test]
fn concurrent_mutations_keep_one_timer_and_every_logged_session() {
    const WRITERS: usize = 6;
    const LOGS_PER_ITEM: usize = 3;
    let fixture = Arc::new(Fixture::new());
    let ids: Vec<String> = (0..WRITERS)
        .map(|writer| fixture.add(&format!("item {writer}")))
        .collect();

    let readers = Readers::spawn(Arc::clone(&fixture), 3);

    let mut handles = Vec::new();
    for (writer, id) in ids.iter().enumerate() {
        let fixture = Arc::clone(&fixture);
        let id = id.clone();
        handles.push(thread::spawn(move || {
            for round in 0..2 {
                let index = ((writer + round) % WRITERS).to_string();
                let args = strings([
                    "item", "move", &id, "--block", "asr", "--index", &index, "--json",
                ]);
                let output = fixture.run(&args);
                assert_success(&output, &args);
            }
            for _ in 0..LOGS_PER_ITEM {
                let args = strings(["session", "log", "--item", &id, "--minutes", "5", "--json"]);
                let output = fixture.run(&args);
                assert_success(&output, &args);
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }

    // Starts race: exactly one wins, the others are refused as conflicts.
    let mut starts = Vec::new();
    for id in &ids {
        let fixture = Arc::clone(&fixture);
        let id = id.clone();
        starts.push(thread::spawn(move || {
            let output = fixture.run(&strings(["session", "start", "--item", &id, "--json"]));
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(
                    stderr.contains("already running"),
                    "unexpected failure: {stderr}"
                );
            }
            output.status.success()
        }));
    }
    let started = starts
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .filter(|started| *started)
        .count();

    readers.finish();

    assert_eq!(started, 1, "exactly one concurrent start succeeds");
    let store = fixture.store();
    assert_eq!(store.running_sessions().unwrap().len(), 1);
    let positions = live_positions(&store, BoundaryId::Asr);
    assert_eq!(positions.len(), WRITERS);
    assert_dense(&positions);

    let day = chrono::NaiveDate::parse_from_str(DATE, "%Y-%m-%d").unwrap();
    let sessions = store.sessions_between(day, day).unwrap();
    for id in &ids {
        let logged = sessions
            .iter()
            .filter(|s| s.is_live() && s.item_id.as_deref() == Some(id) && s.minutes == Some(5))
            .count();
        assert_eq!(
            logged, LOGS_PER_ITEM,
            "every successful log command must keep exactly one session"
        );
    }
}
