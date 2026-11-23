# C1 tracking backend

## Task 1 — review and regressions

Locations below refer to the code before this checkpoint (architecture, `9bcc596`).
The RPC contract fixtures pass with their existing contents.

| Finding and original location | Failing scenario | Fix and regression coverage |
|---|---|---|
| Replay after body validation, `crates/salah-protocol/src/writes.rs:21`, `:148`, `:368` | Retry an applied `config.set` with just the same op ID. The server rejected the empty body. Reusing an existing op ID on another method with missing fields also returned the wrong field. | Check for replay before parsing the mutation body or loading config. Recheck under the write lock for racing requests. Dispatcher tests cover exact replay, cross-method rejection, revision and empty IDs. |
| Routine prompt, `crates/salah-core/src/sessions.rs:138` | Start a timer on an item without a project. Status supplied no focus or quality controls. | A running routine gets the two optional built-ins. Idle status keeps its empty prompt. A dispatcher test exercises start, status and stop without ratings. |
| Archived value loss, `crates/salah-core/src/sessions.rs:343` | Archive Pages after recording 12. Edit the session using just the fields still visible in its prompt. Replacing the values map silently deleted Pages. | Preserve omitted archived samples, accept an unchanged sample and reject a changed or newly supplied archived one. Covered by the archived-metric core test. |
| In-progress session edits, `crates/salah-core/src/sessions.rs:254`, `:192` | Enter just focus and a note while the timer runs, then enter quality on stop. The edit required both ratings and stop discarded previously entered values and notes. | Validate each supplied value immediately, enforce required fields at finish and retain the saved note when omitted on stop. A dispatcher test covers partial entry, a rejected stop and a successful retry. |
| Sleep clamp, `crates/salah-core/src/sessions.rs:185`, `crates/salah-prayer/src/schedule.rs:179`, `crates/salah-domain/src/day.rs:222` | With Sleep at 14:00, a 13:00 timer stopped at 21:00 counted past Sleep. Later prayers reopened the previous planning day. Waking capacity also included time after Sleep. | Carry the actual Sleep instant, end waking capacity at Sleep and keep Night current until Fajr. Core coverage checks the 60-minute result, end instant, planning date, displayed Sleep and block capacities. |
| Incomplete historical plans, `crates/salah-core/src/day/snapshot.rs:60`, `crates/salah-core/src/day/view.rs:325` | Read yesterday after a long gap, then rename and reassign a daily item. The latest-written-day cursor skipped earlier holes. Unread days beyond the catch-up window acquired the new recurrence and project. Frozen rows also displayed the new title and recurrence. | Check actual written days in the bounded window. Retain the first pre-edit item state per planning day for historical reconstruction, including ordering and removal. The sparse-read regression checks both a written day and an unread day over a year old. |
| Clock history and first writes, `crates/salah-core/src/day/view.rs:156`, `:162`, `crates/salah-protocol/src/writes.rs:376`, `crates/salah-core/src/location.rs:95` | Change location and UTC offset. Frozen prayer and Sleep labels shifted, sunrise was recalculated, and an unread past day used the new settings. Stopping, editing or removing a past session or marking a past prayer also left its day unwritten. | Persist local clock metadata with a frozen day. Keep previous settings for unread days on manual updates and successful location sync. Materialize the affected past day inside the write. Core, dispatcher and v1 store-upgrade tests cover these paths and retained data. |
| Day and stats totals, `crates/salah-core/src/stats.rs:183`, `:345`, `:372` | A tick-only day counted as inactive. Moving a plan line from project A to B made the original A session count as unplanned in the A filter. A count of 1e308 with a ten-minute weight overflowed before division and produced a null average. | Count days with any finished session, classify unplanned work against the whole day's plan, and compute the weighted mean incrementally. Tests include a project filter, a session done in another block, a bare-project log, ticks and large finite counts. |
| One-time item recurrence, `crates/salah-domain/src/tracking.rs:201` | List yesterday's one-time item today. It advertised today as its next occurrence. | Return null after its one applicable day. Domain assertions cover both sides of the date. |
| Notification delivery, `crates/salah-cli/src/daemon.rs:148`, `:203`, `:418`, `:682` | A clock-only subscriber received changes. An update between subscription revision lookup and registration could be lost. Splitting a tag removal across watcher pages emitted one revision twice, which let revision deduplication hide the tag scope. A read failure advanced the poll version before publication. | Register atomically with the revision lookup, filter by topic, establish the watcher cursor before serving and group whole revisions across pages. A socket-free test runs every project/tag/metric/session mutation through dispatch and the watcher with one change per page. Removing an attached tag emits both projects and tags. A second test covers subscription replacement. Config writes also emit the documented config scope. |
| SQLite recovery, `crates/salah-store/src/lib.rs:177`, `:200` | A reader holding a shared lock can make COMMIT return busy and leave the writer inside its transaction. Concurrent first opens could read schema presence before initialization and version after it, then set aside the newly initialized database. | Use transaction rollback on failed commit and read the two schema facts in one SQLite statement. Tests force a busy commit and reuse that connection, and verify concurrent first opens retain all writes without creating an old-database backup. |

The review also checked scale/count/check validation, one running timer, item-to-session project copying, required and optional project metrics, archived metrics in prompts, version conflicts, running and removed session exclusion, minute rounding, fulfilment ratios, Night capacity and tag filtering. Project prompts retain the project's configured metric order under the task 1 contract.

### Compatibility

The existing tracking-v1 database receives additive history tables on open. Entity IDs, applied operations and existing day snapshots survive. Older databases did not record the original timezone or sunrise. For those snapshots, the first subsequent read fills the missing metadata from the earliest applicable settings available. Item edits that predate history capture cannot be reconstructed if their original fields were never stored.

### Verification

- 129 non-socket Rust tests passed, including golden RPC fixtures, CLI subprocesses, concurrency, dispatcher and watcher tests.
- `cargo test --offline --workspace` failed on all eight tests in `crates/salah-cli/tests/daemon.rs`. Unix socket bind returned EPERM in the sandbox. Seven tests timed out waiting for the listener, and the stale-socket test failed at its own bind. A full rerun with `--no-fail-fast` confirmed this was the only failing target.
- `cargo clippy --offline --all-targets -- -D warnings`, `cargo fmt --check` and `git diff --check` passed.

### Checkpoint files

- `crates/salah-cli/src/daemon.rs`, `crates/salah-cli/src/daemon/events.rs`, `crates/salah-cli/tests/concurrency.rs`
- `crates/salah-core/src/config.rs`, `crates/salah-core/src/day/snapshot.rs`, `crates/salah-core/src/day/view.rs`, `crates/salah-core/src/items.rs`, `crates/salah-core/src/location.rs`, `crates/salah-core/src/marks.rs`, `crates/salah-core/src/sessions.rs`, `crates/salah-core/src/stats.rs`, `crates/salah-core/src/status.rs`
- `crates/salah-domain/src/day.rs`, `crates/salah-domain/src/tracking.rs`, `crates/salah-prayer/src/schedule.rs`
- `crates/salah-protocol/src/dispatcher.rs`, `crates/salah-protocol/src/message.rs`, `crates/salah-protocol/src/writes.rs`, `crates/salah-protocol/tests/dispatch.rs`
- `crates/salah-store/src/codec.rs`, `crates/salah-store/src/history.sql`, `crates/salah-store/src/lib.rs`, `crates/salah-store/src/read.rs`, `crates/salah-store/src/write.rs`
- `docs/work/tracking-ui/c1-progress.md`

CHECKPOINT 1: Preserve tracking history and harden session validation, stats, replay and notifications

## User decisions — optional metrics and explicit completion

`session.stop`, `session.log` and `session.edit` accept empty or partial metric values. Supplied values retain kind, range and archive validation. Project bindings retain their stored `required` flag for compatibility. All entries in `status.get.prompt` return `required: false`.

A live tick makes an item done on the tick's day. Timed work continues to count toward minutes and the fulfilment ratio. Ticking the item whose timer is running stops that timer first inside the same write transaction. The RPC result lists the new tick first for existing log callers, followed by the stopped timer. Each session keeps its own day and values. A retry returns both original rows at the original revision.

Regression coverage includes an overnight 810-minute Sleep clamp, a tick with invalid values leaving the timer running, unrelated item and project ticks, manual minute logs during a timer, distinct tick and timer values, one revision and op ID for both changes, replay, and unticking while retaining the timed work.

### Verification

- Regenerated golden responses with `UPDATE_FIXTURES=1 cargo test --offline --test rpc` and verified them in the ordinary suite.
- 130 non-socket Rust tests passed. All eight tests in `crates/salah-cli/tests/daemon.rs` failed in the full workspace run because this sandbox denies Unix socket creation.
- Strict Clippy, `cargo fmt --check`, `git diff --check`, and the widget's 35 session-logic checks passed.
- UI-A integration follow-up at `app/tests/salah_app_tests.cpp:806`: the test reads the deleted missing-metrics error fixture. The replacement success assertion can read `session_stop.json` and check the stopped session's empty values.
- Original task 2 is obsolete because the missing-metrics error was removed. Tasks 3–5 remain queued for subsequent checkpoints.

### Checkpoint files

- `crates/salah-domain/src/{error,tracking}.rs`
- `crates/salah-core/src/{lib,projects,sessions}.rs`, `crates/salah-core/src/day/{fulfilment,view}.rs`
- `crates/salah-protocol/src/{error,writes}.rs`, `crates/salah-protocol/tests/dispatch.rs`
- `crates/salah-cli/src/{render,tracking}.rs`, `crates/salah-cli/tests/{daemon,rpc,tracking_cli}.rs`
- `crates/salah-cli/tests/fixtures/rpc/{day_tracking,session_edit,session_log,session_log_tick_stops_timer,session_stop,session_stop_invalid_value,stats,stats_by_tag,status_running}.json`; removed `session_stop_missing_metrics.json` from the same directory.
- `docs/work/architecture/protocol-v1.md`, `docs/work/tracking-ui/c1-progress.md`

CHECKPOINT A-D: Make session metrics optional and record explicit completion ticks atomically with timer stops

## Task 3 — stale running timers

`SessionView.stale` becomes true strictly after an open timer's planning-day Sleep instant. Status, day rows, and session mutation results use the same boundary as `session.stop`. The timer stays available to stop, and a late stop records clamped minutes. Finished sessions and ticks return false. If status can read the timer but cannot resolve its clock, it preserves the timer and prompt, omits `stale`, and reports the clock error in `planner_error`. Stored operation replays retain their original result.

The stdio RPC regression starts a timer at 09:30 with Sleep at 00:30 the next civil day. It checks one second before, exactly at, and one second after Sleep in both status and the historical day row. After changing Sleep to 22:00, it checks that editing the timer reports stale, replaying its start returns the original fresh result, and stopping records 900 minutes at the original Sleep instant.

### Verification

- Regenerated RPC fixtures. All four RPC tests passed. A JSON comparison against the committed fixtures verified that the only addition in eight existing files is `stale`. The new `status_stale.json` exercises the true case.
- 131 non-socket Rust tests passed. All eight tests in `crates/salah-cli/tests/daemon.rs` failed during Unix socket setup in the sandbox. Strict Clippy, formatting, and diff checks passed.

### Checkpoint files

- `crates/salah-core/src/day/{mod,snapshot,view}.rs`, `crates/salah-core/src/{sessions,status}.rs`, `crates/salah-protocol/src/writes.rs`
- `crates/salah-cli/tests/rpc.rs`, `crates/salah-cli/tests/fixtures/rpc/{day_tracking,session_edit,session_log,session_log_tick,session_log_tick_stops_timer,session_start,session_stop,status_running,status_stale}.json`
- `docs/work/architecture/protocol-v1.md`, `docs/work/tracking-ui/c1-progress.md`

CHECKPOINT 3: Report stale running timers at their planning-day Sleep cutoff

## Task 4 — snapshot catch-up cost

Item add, edit, move, and remove pass their open write transaction to `freeze_past` in `crates/salah-core/src/items.rs` (lines 132, 181, 228, and 269). The lower bound in `crates/salah-core/src/day/snapshot.rs:110` limits catch-up to the preceding 366 days. The store commits or rolls back the snapshots and item change together in `crates/salah-store/src/lib.rs:167`.

A new CLI regression seeds 24 daily items in a file-backed SQLite store, advances 400 days, then times an item edit through commit. Five debug runs took 208–214 ms against a one-second limit. The test checks the exact snapshot range, the original plan at both ends, and a single revision increment. An invalid edit five days later rolls back that catch-up, including day markers, clock, blocks, and plan. The existing implementation met the target.

132 non-socket Rust tests passed. All eight tests in `crates/salah-cli/tests/daemon.rs` failed at Unix socket setup in the full workspace run under sandbox restrictions. Strict Clippy, formatting, and diff checks passed.

### Checkpoint files

- `crates/salah-cli/tests/tracking_cli.rs`
- `docs/work/architecture/protocol-v1.md`
- `docs/work/tracking-ui/c1-progress.md`

CHECKPOINT 4: Verify bounded, atomic snapshot catch-up after a 400-day gap
