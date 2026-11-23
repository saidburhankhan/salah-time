# C5 progress

## Migration 004

The store backs up v3 and rebuilds `items` within the migration transaction. It copies every v3 column and row ID, restores the project index, and checks foreign keys before committing v4. Existing items, sessions and frozen plan rows receive a null `epic_id`. The session column enforces the project requirement with a column CHECK, which SQLite can add without rebuilding `sessions`.

The migration tests compare every populated v3 table with both its backup and the upgraded scratch database. The seed includes all repeat kinds, custom item metrics, running and finished sessions, ticks, values, session notes, tombstones, frozen days, marks, history and replay records. The tests verify index columns, parent references, backlog pairing, epic status and resource constraints. An intentionally broken item-history reference makes the upgrade roll back to v3 with foreign keys enabled. Repairing the reference allows the same file to migrate successfully.

### Verification

All 18 store tests passed. Across the workspace, `cargo test --offline --workspace --no-fail-fast` passed 183 tests. The sandbox denied Unix socket setup for these eight tests in `crates/salah-cli/tests/daemon.rs`:

- `a_second_daemon_refuses_to_steal_the_live_socket`
- `a_stale_socket_file_is_replaced_and_sigterm_removes_it`
- `clock_boundary_is_pushed_when_the_next_boundary_changes`
- `concurrent_writers_keep_revisions_gapless`
- `every_protocol_method_has_a_success_and_an_error_over_the_socket`
- `rpc_socket_bridges_stdio_to_the_daemon`
- `socket_permissions_and_handshake`
- `subscribers_receive_changed_for_socket_and_plain_cli_writes`

`cargo clippy --offline --all-targets -- -D warnings`, `cargo fmt --check` and `git diff --check` passed.

### Files

- `crates/salah-store/src/migrations.rs`
- `crates/salah-store/src/migrations/004_epics_backlog_resources.sql`
- `crates/salah-store/src/migrations/tests.rs`
- `crates/salah-store/src/migrations/tests/phase3.rs`
- `docs/work/tracking-ui/c5-progress.md`

CHECKPOINT 0: Migrate tracking databases in place to schema 004 for optional epics, backlog items and resources

## Epics (2b), review follow-ups, parser day

Taken over from the Codex run that hit its limit mid-slice; its uncommitted epic work was kept
after review and finished.

- Epics: `epics.list {project_id?, status?}` (totals, days worked, last worked, open/done
  tasks), `epic.add`, `epic.edit` (finishing sets `finished_at`, reopening clears it),
  `epic.remove` (soft delete only without sessions, else `conflict` "finish it instead"; live
  items are detached in the same revision). Items carry `epic_id`; the store rejects an epic of
  another project; a project change clears it. Sessions copy `project_id`/`epic_id` when
  written (`put_session` no longer rewrites ownership on update); `session.log/start
  {epic_id}` takes the project from the epic. Frozen plan lines keep their epic.
- Stats: shared filter `epic_id` (incl. `"none"`), `stats.table group:"epic"`,
  `stats.series split:"epic"` with a "No epic" row, `stats.get.epics` (per project, its epics
  most minutes first, then its "No epic" row, project-less work last) and a "No project" row
  in `stats.get.projects` (`color` is null there). `changed` scope `epics`.
- CLI: `salah epic list|add|edit|finish|reopen|remove`; `--epic` on `item add/edit`
  (`--no-epic`), `session start/log`, `stats` and the report scope (`stats table|series`,
  `sessions list`).
- Review follow-ups: the foreign_key_check error names table, rowid, parent and fkid; a
  v1→v4 test asserts the kept backups are exactly v1, v2, v3 and the older copies rotate out.
- Parser: "today"/"tomorrow" set `start_day` on `item.parse` (only then is the tracking day
  resolved) and drop the word; `item.add quick_text` uses it unless `date` is given.

Fixtures: existing ones only gain fields (`epic_id`, `epics`, `start_day`) plus the
"No project" row in `stats.json`. `stats_series_day` and `stats_table_week` regenerate with a
last-digit float difference because sessions created at the same instant tie-break on random
ids (the checker rounds to 1e-9); the committed literals were kept.

### Verification

`cargo test --offline --workspace`: 197 passed, socket tests included (run outside the Codex
sandbox). `cargo clippy --offline --all-targets -- -D warnings`, `cargo fmt --check`,
`git diff --check` pass.

### Files

- `crates/salah-cli/src/daemon/events.rs`, `crates/salah-cli/src/epics.rs`,
  `crates/salah-cli/src/main.rs`, `crates/salah-cli/src/render.rs`,
  `crates/salah-cli/src/reports.rs`, `crates/salah-cli/src/tracking.rs`
- `crates/salah-cli/tests/rpc.rs`, `crates/salah-cli/tests/rpc/epics.rs`,
  `crates/salah-cli/tests/rpc/phase2.rs`, `crates/salah-cli/tests/rpc/quick.rs`,
  `crates/salah-cli/tests/rpc/reports.rs`, `crates/salah-cli/tests/tracking_cli.rs`
- new fixtures in `crates/salah-cli/tests/fixtures/rpc/`: `epic_add.json`,
  `epic_edit_finish.json`, `epics_list.json`, `item_add_quick_tomorrow.json`,
  `item_parse_tomorrow.json`, `session_log_epic.json`, `stats_table_epic.json`
- changed fixtures in `crates/salah-cli/tests/fixtures/rpc/`: `day_tracking.json`,
  `item_add.json`, `item_add_quick.json`, `item_edit.json`, `item_edit_metrics.json`,
  `item_move.json`, `item_parse.json`, `item_remove.json`, `items_list.json`,
  `metric_add.json`, `metric_edit.json`, `prayer_mark.json`, `project_add.json`,
  `project_edit.json`, `session_bump.json`, `session_edit.json`,
  `session_edit_remove_note.json`, `session_log.json`, `session_log_tick.json`,
  `session_log_tick_stops_timer.json`, `session_note.json`, `session_remove.json`,
  `session_start.json`, `session_stop.json`, `session_stop_done.json`,
  `session_stop_minutes.json`, `sessions_list.json`, `sessions_list_filtered.json`,
  `stats.json`, `stats_by_tag.json`, `stats_item_filter.json`, `stats_metric_filter.json`,
  `stats_series_day.json`, `stats_series_split_item.json`, `stats_table_metric_filter.json`,
  `stats_table_project.json`, `stats_table_week.json`, `status_running.json`,
  `status_stale.json`, `tag_add.json`, `tag_edit.json`, `tag_remove.json`
- `crates/salah-core/src/day/snapshot.rs`, `crates/salah-core/src/day/view.rs`,
  `crates/salah-core/src/epics.rs`, `crates/salah-core/src/items.rs`,
  `crates/salah-core/src/lib.rs`, `crates/salah-core/src/quick.rs`,
  `crates/salah-core/src/sessions.rs`, `crates/salah-core/src/sessions/list/tests.rs`,
  `crates/salah-core/src/sessions/running.rs`, `crates/salah-core/src/sessions/tests.rs`,
  `crates/salah-core/src/stats.rs`, `crates/salah-core/src/stats/series.rs`,
  `crates/salah-core/src/stats/table.rs`, `crates/salah-core/src/stats/table/tests.rs`,
  `crates/salah-core/src/status.rs`
- `crates/salah-domain/src/epic.rs`, `crates/salah-domain/src/lib.rs`,
  `crates/salah-domain/src/tracking.rs`
- `crates/salah-protocol/src/dispatcher.rs`, `crates/salah-protocol/src/message.rs`,
  `crates/salah-protocol/src/methods.rs`, `crates/salah-protocol/src/reads.rs`,
  `crates/salah-protocol/src/writes.rs`
- `crates/salah-store/src/codec.rs`, `crates/salah-store/src/epics.rs`,
  `crates/salah-store/src/lib.rs`, `crates/salah-store/src/migrations.rs`,
  `crates/salah-store/src/migrations/tests.rs`,
  `crates/salah-store/src/migrations/tests/phase3.rs`, `crates/salah-store/src/read.rs`,
  `crates/salah-store/src/reports.rs`, `crates/salah-store/src/write.rs`
- `docs/work/architecture/protocol-v1.md`, `docs/work/tracking-ui/c5-progress.md`

CHECKPOINT 2b: Optional epics in projects with epic stats, CLI and "No project"/"No epic" rows; parser reads today/tomorrow; migration review follow-ups

## Backlog (1), with the 2b follow-ups

Follow-ups from the 2b review (orchestrator answers):
- `sessions.list` rows gain `epic: {id, name}|null`.
- A finished epic rejects new tasks and tasks moved into it (`validation` on `epic_id`,
  "epic is finished; reopen it to add tasks"); its existing tasks keep it and late sessions
  are still accepted.
- An epic given without a project brings its project (`item.add`, and `item.edit` of an item
  without a project). This fixes `salah item add --quick --epic <id> "..."`, which clap
  rejected with an empty "required arguments" message; `--epic` no longer requires `--project`.

Backlog, as plan.md Phase 3 and data-model.md 004 Rules say:
- `Item.block`/`Item.day` (codec `block`/`start_day`) are `Option`; `recurs_on` and
  `next_occurrence` treat a backlog item as planned on no day, so it never reaches
  `day_plan`, snapshots, capacity or fulfilment. Item history JSON written before decodes
  unchanged (domain test), and a backlog item round-trips through the store.
- `item.add {backlog: true}` (one-time only; explicit `block`/`date` rejected, a quick text's
  parsed day/block dropped), `item.schedule {id, day, block}` (from today on; also moves a
  planned one-time task), `item.backlog {id}` (idempotent; a past day that lists the task
  keeps it), `item.move {block: null}` reorders the backlog, and a move across the
  backlog/block line, a block edit or a repeat on a backlog item are validation errors.
- `backlog.list {project_id?, tag_id?}` (each also `"none"`) returns ItemView + `project:
  {id,name,color,tag_ids}|null` + `resources: {notes, links, reminders}` (live resources,
  read from the 004 table; resource methods come with task 2). `ItemView.backlog` is new.
- CLI: `salah backlog list|add|schedule`; the item renderings show "backlog".
- The socket method test also covers `backlog.list`, `item.schedule`, `item.backlog` and
  `epics.list`.

Fixtures: existing ones only gain `backlog: false` and `epic: null`
(`git diff -U0` shows no removed line). New: `backlog_list.json`, `item_add_backlog.json`,
`item_schedule.json`, `item_backlog.json`.

### Verification

`cargo test --offline --workspace`: 202 passed, socket tests included. `cargo clippy
--offline --all-targets -- -D warnings`, `cargo fmt --check`, `git diff --check` pass.
`crates/salah-cli/tests/daemon.rs` was already over 800 lines (970) and is now 1007.

### Files

- `crates/salah-cli/src/backlog.rs`, `crates/salah-cli/src/main.rs`,
  `crates/salah-cli/src/render.rs`, `crates/salah-cli/src/tracking.rs`
- `crates/salah-cli/tests/concurrency.rs`, `crates/salah-cli/tests/daemon.rs`,
  `crates/salah-cli/tests/rpc.rs`, `crates/salah-cli/tests/rpc/backlog.rs`,
  `crates/salah-cli/tests/rpc/epics.rs`, `crates/salah-cli/tests/tracking_cli.rs`
- new fixtures in `crates/salah-cli/tests/fixtures/rpc/`: `backlog_list.json`,
  `item_add_backlog.json`, `item_backlog.json`, `item_schedule.json`
- changed fixtures in `crates/salah-cli/tests/fixtures/rpc/`: `item_add.json`,
  `item_add_quick.json`, `item_add_quick_tomorrow.json`, `item_edit.json`,
  `item_edit_metrics.json`, `item_move.json`, `items_list.json`, `sessions_list.json`,
  `sessions_list_filtered.json`
- `crates/salah-core/src/backlog.rs`, `crates/salah-core/src/day/snapshot.rs`,
  `crates/salah-core/src/epics.rs`, `crates/salah-core/src/items.rs`,
  `crates/salah-core/src/lib.rs`, `crates/salah-core/src/sessions.rs`,
  `crates/salah-core/src/sessions/list.rs`
- `crates/salah-domain/src/epic.rs`, `crates/salah-domain/src/lib.rs`,
  `crates/salah-domain/src/resource.rs`, `crates/salah-domain/src/tracking.rs`
- `crates/salah-protocol/src/dispatcher.rs`, `crates/salah-protocol/src/methods.rs`,
  `crates/salah-protocol/src/reads.rs`, `crates/salah-protocol/src/writes.rs`,
  `crates/salah-protocol/tests/dispatch.rs`
- `crates/salah-store/src/codec.rs`, `crates/salah-store/src/lib.rs`,
  `crates/salah-store/src/resources.rs`, `crates/salah-store/src/write.rs`
- `docs/work/architecture/protocol-v1.md`, `docs/work/tracking-ui/c5-progress.md`

CHECKPOINT 1: Backlog for one-time tasks (add, schedule, back to backlog, reorder, list) kept out of every plan; finished epics take no new tasks; an epic brings its project; sessions.list names the epic
