# C2 progress

## Task 0 — migration chain through version 3

The store applies each numbered migration and its version bump in one transaction. Before each file-backed step it takes a private SQLite backup under the writer lock, including committed WAL data, and keeps the latest three copies. Opening a current database creates no backup. The store refuses newer versions and pre-tracking files in place.

The three SQL files match the original schema, original history SQL and locked 003 SQL byte for byte. The migration tests compare the original columns of every seeded table, covering all repeat kinds, sessions and values, frozen days, marks, history, replay records and relations. They check each pre-step backup, foreign keys, integrity, default metric selections, note constraints, rollback, retry and backup retention. The three regenerated RPC fixtures gain only the new built-in metric or its series.

### Verification

138 tests passed with `cargo test --offline --workspace --no-fail-fast`. The sandbox prevented Unix socket setup for these eight tests in `crates/salah-cli/tests/daemon.rs`:

- `a_second_daemon_refuses_to_steal_the_live_socket`
- `a_stale_socket_file_is_replaced_and_sigterm_removes_it`
- `clock_boundary_is_pushed_when_the_next_boundary_changes`
- `concurrent_writers_keep_revisions_gapless`
- `every_protocol_method_has_a_success_and_an_error_over_the_socket`
- `rpc_socket_bridges_stdio_to_the_daemon`
- `socket_permissions_and_handshake`
- `subscribers_receive_changed_for_socket_and_plain_cli_writes`

Strict Clippy, formatting and diff checks passed. All migration runs used scratch databases.

### Checkpoint files

- `crates/salah-store/Cargo.toml`, `crates/salah-store/src/{lib,read,migrations}.rs`
- `crates/salah-store/src/schema.sql` → `crates/salah-store/src/migrations/001_schema.sql`
- `crates/salah-store/src/history.sql` → `crates/salah-store/src/migrations/002_history.sql`
- `crates/salah-store/src/migrations/{003_tracking_ui.sql,backup.rs,tests.rs,tests/tracking_v1.sql}`
- `crates/salah-cli/tests/fixtures/rpc/{metrics_list,stats,stats_by_tag}.json`
- `docs/work/tracking-ui/c2-progress.md`

CHECKPOINT 0: Migrate tracking databases in place through schema 003 with per-step SQLite backups

## Task 0b — independent review fixes

A failed migration or failed commit discards its new backup. Successful commits prune the oldest copies. File-backed migrators hold a directory lock across the SQLite commit and backup cleanup so concurrent opens cannot rotate a backup that another step is about to discard. Fresh databases skip the v0 backup. Saving a backup removes stale partial copies matching that database's backup naming convention.

Steps now declare whether a table rebuild needs foreign-key enforcement disabled. The runner sets the pragma before beginning the transaction, checks every foreign key before committing the rebuild, and restores enforcement on success and error. Pre-tracking schema errors identify the version, the missing tracking table, and that the file was left untouched.

Regression tests cover four failed retries preserving a v1 backup byte for byte, a busy commit preserving three older backups, and a successful v1-to-v3 run rotating two real pre-existing backups. The row-preservation fixture includes a soft-deleted item and its soft-deleted session with a metric value. A test-only rebuild exercises successful replacement of a referenced table, an orphan detected before commit, and a SQL failure. The production migration chain ends at 003.

New projects ask for distractions after focus and quality. The new binding has `required: false`. The RPC fixture, protocol and project prompt expectations reflect the default.

### Verification

140 workspace tests passed, including all 15 store tests and all four stdio RPC fixture tests. The eight daemon socket tests listed at checkpoint 0 failed at socket setup in this sandbox. Strict Clippy, formatting and diff checks passed. All three migration SQL files still match their locked sources byte for byte.

### Files changed since checkpoint 0

- `crates/salah-store/src/{lib,migrations}.rs`
- `crates/salah-store/src/migrations/{backup.rs,tests.rs,tests/tracking_v1.sql}`
- `crates/salah-core/src/{projects,sessions,status}.rs`
- `crates/salah-cli/tests/tracking_cli.rs`, `crates/salah-cli/tests/fixtures/rpc/project_add.json`
- `docs/work/architecture/protocol-v1.md`, `docs/work/tracking-ui/c2-progress.md`

CHECKPOINT 0b: Preserve good backups across failed migrations, validate table rebuilds and default new projects to optional distractions

## Tasks 1–2d — stats filters and session writes

- `stats.get` intersects the item, project, and tag filters and accepts up to 730 inclusive planning days. Item filtering narrows planned minutes and fulfilment. Historical sessions can still be queried by a removed item's ID.
- `session.stop` accepts an override up to the Sleep-clamped, rounded elapsed duration. The end instant follows the original timer cutoff. Save & Done returns the timed session first and the completion tick second. Completion uses the timer's planning day and reuses an existing tick on that day. The stopped session receives the supplied values and main note. Both new writes share the replay record and revision. The CLI supports `session stop --done --minutes N` and `stats --item ID`.
- `session.bump` updates one active count value on the running timer and floors it at zero. Distractions are available even for a task with an empty prompt. A pause that omits the count retains it, and finished counts appear in the stats series.
- Item views and day rows expose the stored `metric_ids` and effective prompt. The running timer uses its current task's prompt. A project change resets the selection to inheritance, and a project metric removal strips that ID from custom selections in the same write. Historical day rows display their frozen plan and use the current metric selection for session input.
- `session.note` appends a separate session note and returns it in `SessionView.notes`. Notes are ordered oldest first. Editing can soft-delete named entries, and removing a session soft-deletes its live entries. Note and count mutations advance the session revision and replay by `op_id`.

The derived distractions-per-hour column belongs to the upcoming `stats.table` slice. The Notes page query is also in the upcoming slice.

### Verification

145 tests passed with `cargo test --offline --workspace --no-fail-fast`. The same eight tests listed at checkpoint 0 in `crates/salah-cli/tests/daemon.rs` failed during Unix socket setup. All eight stdio RPC tests passed, including fixture regeneration and a subsequent comparison. A structural comparison verified that existing fixtures gained fields or the new prompt metric while retaining existing values. Strict Clippy, formatting and diff checks passed.

### Checkpoint files

- `crates/salah-cli/src/tracking.rs`, `crates/salah-cli/tests/{rpc.rs,rpc/phase2.rs,tracking_cli.rs}`
- New RPC fixtures in `crates/salah-cli/tests/fixtures/rpc/` (`item_edit_metrics.json`, `session_bump.json`, `session_edit_remove_note.json`, `session_note.json`, `session_stop_done.json`, `session_stop_minutes.json`, `stats_item_filter.json`)
- Updated RPC fixtures in the same directory (`day_tracking.json`, `item_add.json`, `item_add_quick.json`, `item_edit.json`, `item_move.json`, `items_list.json`, `session_edit.json`, `session_log.json`, `session_log_tick.json`, `session_log_tick_stops_timer.json`, `session_start.json`, `session_stop.json`, `stats.json`, `stats_by_tag.json`, `status_running.json`, `status_stale.json`)
- `crates/salah-core/src/{day/view.rs,items.rs,projects.rs,sessions.rs,sessions/running.rs,sessions/tests.rs,stats.rs,status.rs}`
- `crates/salah-domain/src/tracking.rs`
- `crates/salah-protocol/src/{dispatcher,methods,reads,writes}.rs`, `crates/salah-protocol/tests/dispatch.rs`
- `crates/salah-store/src/{codec.rs,lib.rs,migrations/tests.rs,read.rs,write.rs}`
- `docs/work/architecture/protocol-v1.md`, `docs/work/tracking-ui/c2-progress.md`

CHECKPOINT 2d: Add task-filtered stats, atomic Save & Done, live counters, task metrics and timestamped notes
