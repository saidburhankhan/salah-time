# C8 progress

## 0a — shared `block_minutes` rule (finished by Claude from Codex C8's WIP, 2026-11-23)

- `salah_core::day::block_minutes(session, blocks)` (`day/allocation.rs`): a timed session's
  recorded `minutes` (incl. a Worked override) are split over the blocks its real span overlapped,
  proportional to time in each block; whole minutes by largest remainder, ties to the start
  block; parts sum exactly to `minutes`. Logged sessions without times, and timers whose span
  overlaps no block, stay whole in their recorded block. Ticks/running timers have no minutes.
  The old capacity helper is renamed `block_capacity`.
- Day view block bars (`actual_minutes`/`unplanned_minutes`) use it; rows, fulfilment and task
  totals still count each session whole under its start block.
- `stats.table` group `block` and `stats.series` split `block` (`stats/blocks.rs`): per-block
  samples carry the allocated minutes; avg metrics weigh by them, sum metrics scale by the share;
  ticks are one sample in their block; totals count each session once. Past days use frozen
  boundaries (`day::report_blocks`), others the timetable of that day's config. `stats.get`
  `blocks` is unchanged (still whole in the recorded block).
- Tests: 2 h across Fajr→Dhuhr→Asr incl. Worked overrides and remainder ties
  (`day/allocation.rs`), instant timers and manual logs, a forgotten timer clamped at Sleep whose
  block rows match the day bars even after bedtime changes (`sessions/tests.rs`), RPC
  end-to-end table/series/day.get with fixture `stats_table_block.json`.
- Checks: fmt, clippy -D warnings, `cargo test --offline --workspace` 217 passed (socket tests
  included), `git diff --check`. Existing fixtures unchanged.

CHECKPOINT 0a: Core: shared block_minutes rule for day block bars, stats.table group "block" and stats.series split "block" — files: crates/salah-core/src/day/{allocation.rs,mod.rs,snapshot.rs,view.rs}, crates/salah-core/src/stats.rs, crates/salah-core/src/stats/{blocks.rs,series.rs,table.rs,series/tests.rs,table/tests.rs}, crates/salah-core/src/sessions/tests.rs, crates/salah-protocol/src/reads.rs, crates/salah-cli/src/reports.rs, crates/salah-cli/tests/rpc/reports.rs, crates/salah-cli/tests/fixtures/rpc/stats_table_block.json, docs/work/architecture/protocol-v1.md, docs/work/tracking-ui/c8-progress.md

## 0 — migration 005 (Claude standing in for Codex C8, 2026-11-23)

- `crates/salah-store/src/migrations/005_goals_reviews_item_tags.sql`: `goals`, `goals_active`,
  `reviews`, `reviews_by_week` (unique, partial on live rows), `item_tags`, `item_tags_by_tag` and
  the copy of each project's live tags onto every item of that project. Its statements match
  data-model.md "005" as of 4916877 exactly; only header and section comments were added. Wired as step 5 with
  `foreign_keys_off: false`, because nothing is rebuilt. The existing runner handles the rest:
  numbering by user_version, the `salah.db.bak-v4-<ts>` backup, keeping 3 copies, discarding its
  own copy on failure. `schema_version()` comes from `STEPS.len()`, so it is 5 with no other
  change. The core `status::SCHEMA_VERSION` is the status JSON version and is left alone.
- Orchestrator decisions (4916877): the copy joins `tags t ON t.id = pt.tag_id AND
  t.deleted_at IS NULL`, so a `project_tags` link to a removed tag is not copied. Removed tasks
  still get their project's live tags, so their sessions keep their labels (accepted). The
  moved-task relabel described below is accepted and stays pinned by its test.
- Existing migration tests now stop at 004 where they check 004-era backups or `user_version 4`
  (`run(.., &STEPS[..4])`), or compare against `schema_version()`. Their assertions are otherwise
  unchanged. No RPC fixtures changed.
- New `migrations/tests/phase4.rs` plus the fixture `tests/tracking_v4.sql`, on top of the
  v1 and v3 rows. The fixture has a second tagged project with two epics (active and finished)
  and a link to a removed tag, an untagged project, a backlog task, a removed task with live
  work, epic-only work, bare work in the archived project, a no-project task, timed, tick,
  logged and removed sessions, values, notes, a day_plan epic and all resource kinds. The
  tests check:
  every v4 row is byte-identical after the step and in the v4 backup; table counts; integrity and
  foreign-key checks; the exact `item_tags` rows; that they equal items × live project_tags both
  ways, with the removed tag's link left in `project_tags` but not copied; the goals/reviews/item_tags CHECK, NOT NULL, FK and unique rules, with the accepted edges;
  that rerunning the runner, `STEPS[..5]` or reopening is a no-op (no second copy, no new
  backup); rollback plus discarding only its own backup when the step fails after its
  tables exist (a stray `item_tags_by_tag` index), then retry and keep-3 rotation; a full v1→v5 upgrade keeps exactly the v2, v3 and v4 copies.
- Tag totals: the real `stats.table` cannot be run on a v4 file from `salah-core`, because the
  runner is crate-private, and before 0c `stats.table` does not read `item_tags`. So the test
  models both rules in SQL over the same session scope `stats.table` uses: v4 = the live tags of
  the session's project, v5 = the item's tags, or the project's when there is no item. The
  per-session tag sets and the per-tag (sessions, minutes) rows match exactly on the fixture.
  Accepted divergence, pinned by `v5_labels_work_of_a_moved_task_with_its_current_project_tags`:
  a session whose item changed project after it was written (`s.project_id IS NOT i.project_id`)
  takes the item's current project tags. On the live copy these are 6 sessions logged before
  their task had a project; they go from "No tag" to the task's tags.
- Checks: `cargo fmt --all --check`, `cargo clippy --offline --all-targets -- -D warnings`,
  `cargo test --offline --workspace` 223 passed (socket tests included), `git diff --check`.

CHECKPOINT 0: Store: migration 005 (goals, reviews, item_tags seeded from project tags) with v4→v5 migration tests — files: crates/salah-store/src/migrations.rs, crates/salah-store/src/migrations/005_goals_reviews_item_tags.sql, crates/salah-store/src/migrations/tests.rs, crates/salah-store/src/migrations/tests/phase3.rs, crates/salah-store/src/migrations/tests/phase4.rs, crates/salah-store/src/migrations/tests/tracking_v4.sql, docs/work/tracking-ui/c8-progress.md

## 0b — prayer-block totals in `stats.get`

`stats.get.blocks` now uses the same allocated session minutes and focus weights as
`stats.table` and `stats.series`. The RPC regression exercises a Fajr-to-Dhuhr timer
with a Worked override, a manual Dhuhr log, an Asr tick and three empty blocks.
It checks minutes and average focus against the table and verifies the 35-minute total.

Workspace tests passed (215). The sandbox skipped the eight Unix-socket tests in
the CLI daemon target:

- `methods::every_protocol_method_has_a_success_and_an_error_over_the_socket`
- `transport::a_second_daemon_refuses_to_steal_the_live_socket`
- `transport::a_stale_socket_file_is_replaced_and_sigterm_removes_it`
- `transport::clock_boundary_is_pushed_when_the_next_boundary_changes`
- `transport::concurrent_writers_keep_revisions_gapless`
- `transport::rpc_socket_bridges_stdio_to_the_daemon`
- `transport::socket_permissions_and_handshake`
- `transport::subscribers_receive_changed_for_socket_and_plain_cli_writes`

Strict Clippy, `cargo fmt --check` and `git diff --check` passed.

CHECKPOINT 0b: Use shared prayer-block minutes and focus weights in stats.get — files: crates/salah-core/src/stats.rs, crates/salah-core/src/stats/{blocks,table}.rs, crates/salah-cli/tests/rpc/reports.rs, docs/work/architecture/protocol-v1.md, docs/work/tracking-ui/c8-progress.md

## 0c — task tags

Task tags now drive session and backlog filters, table and trend groups, and the tag breakdown. Projects supply defaults at creation. A v4 fixture queried before migration and through stdio RPC after migration preserved all 187 minutes and nine sessions. Moving the course before migration relabelled its 50 minutes and tick as expected. Both CLI and RPC cover explicit sets and clearing.

218 workspace tests passed. The same eight socket tests listed under 0b were filtered out. Strict Clippy, formatting and diff checks passed. Existing fixtures gained only tag_ids fields.

CHECKPOINT 0c: Add task tags, project defaults and read-time session tag reporting — files: crates/salah-cli/src/tracking.rs, crates/salah-cli/tests/fixtures/rpc/backlog_list.json, crates/salah-cli/tests/fixtures/rpc/day_tracking.json, crates/salah-cli/tests/fixtures/rpc/item_add.json, crates/salah-cli/tests/fixtures/rpc/item_add_backlog.json, crates/salah-cli/tests/fixtures/rpc/item_add_quick.json, crates/salah-cli/tests/fixtures/rpc/item_add_quick_tomorrow.json, crates/salah-cli/tests/fixtures/rpc/item_add_tags.json, crates/salah-cli/tests/fixtures/rpc/item_backlog.json, crates/salah-cli/tests/fixtures/rpc/item_edit.json, crates/salah-cli/tests/fixtures/rpc/item_edit_metrics.json, crates/salah-cli/tests/fixtures/rpc/item_move.json, crates/salah-cli/tests/fixtures/rpc/item_schedule.json, crates/salah-cli/tests/fixtures/rpc/items_list.json, crates/salah-cli/tests/fixtures/rpc/stats_table_tag_items.json, crates/salah-cli/tests/fixtures/rpc/status_reminders.json, crates/salah-cli/tests/fixtures/rpc/status_running.json, crates/salah-cli/tests/rpc.rs, crates/salah-cli/tests/rpc/item_tags.rs, crates/salah-cli/tests/tracking_cli.rs, crates/salah-core/src/backlog.rs, crates/salah-core/src/day/view.rs, crates/salah-core/src/items.rs, crates/salah-core/src/projects.rs, crates/salah-core/src/sessions/list/tests.rs, crates/salah-core/src/stats.rs, crates/salah-core/src/stats/table.rs, crates/salah-core/src/stats/table/tests.rs, crates/salah-core/src/tags.rs, crates/salah-protocol/src/methods.rs, crates/salah-protocol/src/writes.rs, crates/salah-store/src/item_tags.rs, crates/salah-store/src/lib.rs, crates/salah-store/src/reports.rs, docs/work/architecture/protocol-v1.md, docs/work/tracking-ui/c8-progress.md
