//! Revision/change-log invariants under real parallel writer processes.
//!
//! Each command runs the actual `salah` binary against one temporary
//! `SALAH_DATA`, matching the concurrency suite.

use salah_core::config::Config;
use salah_store::Store;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

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
        let root = std::env::temp_dir().join(format!(
            "salah-revisions-{}-{}",
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
        Store::open(&data).unwrap();
        Self { root, config, data }
    }

    fn add(&self, title: &str) {
        let output = Command::new(env!("CARGO_BIN_EXE_salah"))
            .env("SALAH_CONFIG", &self.config)
            .env("SALAH_DATA", &self.data)
            .env("SALAH_NOW", NOW)
            .args([
                "item", "add", title, "--block", "asr", "--date", DATE, "--json",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
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

#[test]
fn parallel_writers_produce_gapless_unique_revisions_and_change_rows() {
    const WRITERS: usize = 8;
    const ADDS: usize = 5;
    const TOTAL: usize = WRITERS * ADDS;
    let fixture = Arc::new(Fixture::new());
    let barrier = Arc::new(Barrier::new(WRITERS));

    let handles: Vec<_> = (0..WRITERS)
        .map(|writer| {
            let fixture = Arc::clone(&fixture);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                for add in 0..ADDS {
                    fixture.add(&format!("writer {writer} add {add}"));
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }

    let store = fixture.store();
    assert_eq!(store.revision().unwrap(), TOTAL as i64);

    let changes = store.changes_since(0, TOTAL + 10).unwrap();
    assert_eq!(changes.len(), TOTAL);
    let seqs: Vec<i64> = changes.iter().map(|change| change.seq).collect();
    assert_eq!(seqs, (1..=TOTAL as i64).collect::<Vec<_>>());
    let revisions: HashSet<i64> = changes.iter().map(|change| change.revision).collect();
    assert_eq!(
        revisions,
        (1..=TOTAL as i64).collect::<HashSet<_>>(),
        "every committed write owns exactly one revision"
    );
    for change in &changes {
        assert_eq!(
            (change.entity.as_str(), change.op.as_str()),
            ("item", "insert")
        );
        assert_eq!(change.version, 1);
        assert_eq!(change.op_id, None);
    }
}
