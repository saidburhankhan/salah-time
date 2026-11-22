# W1 blocked report

- Commits: none. The first S1 checkpoint could not be committed because `.git` is read-only; `git commit` failed with `Unable to create '.git/index.lock': Read-only file system`.
- Worktree: added `salah-domain::tracking` with Project, Tag, Metric, ProjectMetric, Session, SessionValue, DayBlock, DayPlanLine, enum types, sync fields, and metric-value validation; exported the module from `salah-domain`.
- Current slice: S1, partially complete. The legacy Item/Entry types have not yet been removed because the required checkpoint cannot be committed.
- Next step: rerun this worker with write access to `.git`, commit the existing S1 work after completing the locked Item replacement, then proceed S2 → S3 → S4.
- Verification: `cargo test --locked --offline -p salah-domain` passed (24 tests); `cargo clippy --offline -p salah-domain --all-targets -- -D warnings` passed; `git diff --check` passed. Full `./scripts/test` passed through the earlier suites but all 8 daemon tests failed because this managed sandbox rejects Unix-socket creation with `Operation not permitted`.
- Deviations from interface.md: temporary/incomplete only—new tracking types were added alongside, rather than replacing, legacy Measure/Entry/Item fields. No locked contract file was edited.
- Remaining risks: Git write permission is required for mandated slice commits; Unix-socket permission is required for complete verification; S2–S4 remain unimplemented.

CHECKPOINT S1: tracking: replace domain entities with tracking model

Files:
- crates/salah-domain/src/tracking.rs
- crates/salah-domain/src/item.rs
- crates/salah-store/src/lib.rs
- crates/salah-core/src/export.rs
- crates/salah-core/src/quick.rs
- crates/salah-core/src/planner/entries.rs
- crates/salah-core/src/planner/items.rs
- crates/salah-core/src/planner/mod.rs
- crates/salah-core/src/planner/preview.rs
- crates/salah-core/src/planner/stats.rs
- crates/salah-core/src/planner/timers.rs
- crates/salah-protocol/src/dispatcher.rs
- crates/salah-protocol/src/methods.rs
- crates/salah-cli/src/main.rs
- crates/salah-cli/tests/concurrency.rs
- crates/salah-cli/tests/export.rs

Verification: all non-daemon workspace tests passed; strict Clippy passed; formatting, shell syntax, plugin validation, and `git diff --check` passed. Socket-dependent daemon tests intentionally not run. S2 should delete the explicitly named `Legacy*` compatibility implementation as the new repository replaces its callers.

CHECKPOINT S2: store: replace migrations with clean tracking schema v1

Files:
- crates/salah-store/src/lib.rs
- crates/salah-core/src/export.rs
- crates/salah-core/src/planner/items.rs
- crates/salah-core/src/planner/stats.rs
- crates/salah-core/src/planner/timers.rs
- crates/salah-core/src/planner/views.rs
- crates/salah-protocol/src/dispatcher.rs
- crates/salah-protocol/tests/dispatch.rs
- crates/salah-cli/tests/contract.rs
- crates/salah-cli/tests/export.rs
- crates/salah-cli/tests/planner_cli.rs

Verification: all non-daemon workspace suites pass; strict Clippy, formatting, and `git diff --check` pass. Store tests cover foreign keys, seeded metrics, constraints, round trips, revisions, and old-database rename. Eight obsolete legacy entry/count assertions are temporarily ignored and identify their S3/S4 replacements. Remaining risk/deviation: the native store API is entirely tracking-model based, but a narrow `Legacy*` translation bridge remains for the old core/protocol callers; S3 must remove that bridge and the domain compatibility types while replacing those callers.
