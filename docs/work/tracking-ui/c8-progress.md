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

## 1 — goals

Inherited from C8's uncommitted work (Codex hit its cap) and finished by Claude.

- Domain `salah_domain::goal`: `Period` (day = tracking day, week = Monday–Sunday,
  `bounds`), `Direction`, `Scope`, `Goal`, `Progress`, `GoalView`, `HistoryRow`.
- Store `salah-store/src/goals.rs`: goal reads, the edit chain (recursive CTE over
  `replaces_id` both ways), `goal_samples` over the same finished live sessions as
  `stats.*`, and writes that validate before SQLite: finite positive target, at most one
  scope id, live scope, the metric active and asked by the scope (task effective metrics,
  project/epic project metrics, built-ins for all work and project-less tasks), minutes
  goals `at_least`, `starts_on`/`ends_on` on period boundaries. Dates are parsed strictly
  as `YYYY-MM-DD` (`goals::parse_day`). No migration change.
- Core `salah-core/src/goals.rs`: progress per period from sessions; minutes sum (a tick
  adds 0, empty = 0); metrics use the metric's rollup through `stats::aggregate` with the
  stats weight (`max(minutes, 1)`, so a tick weighs 1); a metric with no samples is
  `null`/not met (also for `at_most`). Samples are sorted per period so a minutes-weighted
  mean does not depend on the random-id tie order of same-instant sessions. `list`,
  `history` (whole chain, one row per period with that period's target, through today's
  period, max 730 days), `add`, `edit` (successor rule from data-model.md), `end`,
  `remove` (first period only), `end_scope` (item.remove and epic.remove end the scope's
  goals at the last full period, or the first period's end; never deleted) and
  `ending_scope` so those removals list the ended goals in `changed.goals`.
- Protocol: `goals.list`, `goals.history`, `goal.add`, `goal.edit`, `goal.end`,
  `goal.remove`; `status.get.goals` (= `goals.list` for the current tracking day);
  `changed.goals` on every mutation result; change-log entity `goals` → `changed` scope
  `goals`. Documented in protocol-v1.md (method rows, GoalView shape, scopes).
- Decisions: running timers do not count toward progress (same set as stats); `goal.end`
  and scope removal in the first period set `ends_on` to that period's end (the CHECK
  forbids ending before `starts_on`); `goal.edit` re-validates the metric against the
  scope's current asked metrics. Projects have no remove method, so project goals end only
  by `goal.end` (archiving a project leaves them active).
- Tests: `tests/rpc/goals.rs` (scope/no-project/at_most/metric weighting, Sunday 22:59 →
  23:00 Sleep rollover into Monday for day and week goals, mid-week edits keeping history,
  re-edit in the first period, remove/end rules, validation incl. bad dates and ranges,
  item/epic removal ending goals), a socket success/error check for each goal method in
  `tests/daemon/methods.rs`, `Period::bounds` unit test; the goal.add/list fixture test
  now logs each session at its own instant (same-instant sessions made `status_goals.json`
  flaky by id order). `mutations_are_exactly_the_writing_methods` allows `goal.end`.
- Fixtures: new `goal_add`, `goal_edit_successor`, `goals_history`, `goals_list`,
  `status_goals`; existing ones only gained `changed.goals: []` / `status.goals: []`
  (plus `tag_remove.json` items, below).
- Checks: `cargo fmt --all --check`, `cargo clippy --offline --all-targets -- -D warnings`,
  `cargo test --offline --workspace` 230 passed, 0 failed (socket tests included, none
  skipped), `git diff --check`.
- Pre-existing flake noticed, not fixed: `stats_table_week.json` once regenerated with
  focus `2.608695652173913` instead of `…26` — stats' running mean depends on the
  random-id order of sessions with the same `created_at`. 12 reruns passed; left as is.

0c review nits (separate): `tag.remove` lists the live tasks it re-versioned in
`changed.items` (existing `tag_remove.json` `items: []` became that list, the only
non-field-only fixture change); `detach_item_tag` logs `update` for every re-versioned
item (removed ones too) and returns their ids; `item_tag_ids` orders by tag name then id
like `report_tags` (no existing fixture reordered); stale comments fixed in stats.rs
(`Scope::tags`), tags.rs (module doc, `remove` doc). Test: `item_tags.rs` checks name
order and the `tag.remove` items.

CHECKPOINT 1: Goals: domain, store, core progress from sessions, goals.*/goal.* methods, status.get.goals, changed goals, removal ends scope goals — files: crates/salah-cli/tests/daemon/methods.rs, crates/salah-cli/tests/fixtures/rpc/epic_add.json, crates/salah-cli/tests/fixtures/rpc/epic_edit_finish.json, crates/salah-cli/tests/fixtures/rpc/item_add.json, crates/salah-cli/tests/fixtures/rpc/item_add_backlog.json, crates/salah-cli/tests/fixtures/rpc/item_add_quick.json, crates/salah-cli/tests/fixtures/rpc/item_add_quick_tomorrow.json, crates/salah-cli/tests/fixtures/rpc/item_add_tags.json, crates/salah-cli/tests/fixtures/rpc/item_backlog.json, crates/salah-cli/tests/fixtures/rpc/item_edit.json, crates/salah-cli/tests/fixtures/rpc/item_edit_metrics.json, crates/salah-cli/tests/fixtures/rpc/item_move.json, crates/salah-cli/tests/fixtures/rpc/item_remove.json, crates/salah-cli/tests/fixtures/rpc/item_schedule.json, crates/salah-cli/tests/fixtures/rpc/metric_add.json, crates/salah-cli/tests/fixtures/rpc/metric_edit.json, crates/salah-cli/tests/fixtures/rpc/prayer_mark.json, crates/salah-cli/tests/fixtures/rpc/project_add.json, crates/salah-cli/tests/fixtures/rpc/project_edit.json, crates/salah-cli/tests/fixtures/rpc/resource_add_link.json, crates/salah-cli/tests/fixtures/rpc/resource_add_reminder.json, crates/salah-cli/tests/fixtures/rpc/resource_edit_done.json, crates/salah-cli/tests/fixtures/rpc/session_bump.json, crates/salah-cli/tests/fixtures/rpc/session_edit.json, crates/salah-cli/tests/fixtures/rpc/session_edit_remove_note.json, crates/salah-cli/tests/fixtures/rpc/session_log.json, crates/salah-cli/tests/fixtures/rpc/session_log_epic.json, crates/salah-cli/tests/fixtures/rpc/session_log_tick.json, crates/salah-cli/tests/fixtures/rpc/session_log_tick_stops_timer.json, crates/salah-cli/tests/fixtures/rpc/session_note.json, crates/salah-cli/tests/fixtures/rpc/session_remove.json, crates/salah-cli/tests/fixtures/rpc/session_start.json, crates/salah-cli/tests/fixtures/rpc/session_stop.json, crates/salah-cli/tests/fixtures/rpc/session_stop_done.json, crates/salah-cli/tests/fixtures/rpc/session_stop_minutes.json, crates/salah-cli/tests/fixtures/rpc/status.json, crates/salah-cli/tests/fixtures/rpc/status_reminders.json, crates/salah-cli/tests/fixtures/rpc/status_running.json, crates/salah-cli/tests/fixtures/rpc/status_stale.json, crates/salah-cli/tests/fixtures/rpc/tag_add.json, crates/salah-cli/tests/fixtures/rpc/tag_edit.json, crates/salah-cli/tests/fixtures/rpc/tag_remove.json, crates/salah-cli/tests/rpc.rs, crates/salah-core/src/epics.rs, crates/salah-core/src/items.rs, crates/salah-core/src/lib.rs, crates/salah-core/src/stats.rs, crates/salah-core/src/status.rs, crates/salah-domain/src/lib.rs, crates/salah-protocol/src/dispatcher.rs, crates/salah-protocol/src/lib.rs, crates/salah-protocol/src/message.rs, crates/salah-protocol/src/methods.rs, crates/salah-protocol/src/writes.rs, crates/salah-protocol/tests/dispatch.rs, crates/salah-store/src/codec.rs, crates/salah-store/src/lib.rs, docs/work/architecture/protocol-v1.md, crates/salah-cli/tests/fixtures/rpc/goal_add.json, crates/salah-cli/tests/fixtures/rpc/goal_edit_successor.json, crates/salah-cli/tests/fixtures/rpc/goals_history.json, crates/salah-cli/tests/fixtures/rpc/goals_list.json, crates/salah-cli/tests/fixtures/rpc/status_goals.json, crates/salah-cli/tests/rpc/goals.rs, crates/salah-core/src/goals.rs, crates/salah-domain/src/goal.rs, crates/salah-protocol/src/goals.rs, crates/salah-store/src/goals.rs, docs/work/tracking-ui/c8-progress.md
CHECKPOINT 0c-nits: tag.remove lists re-versioned tasks, detach logs update, item tag_ids in name order, stale tag comments — files: crates/salah-store/src/item_tags.rs, crates/salah-core/src/tags.rs, crates/salah-core/src/stats.rs (comment only), crates/salah-protocol/src/writes.rs (tag_remove), crates/salah-cli/tests/rpc/item_tags.rs, crates/salah-cli/tests/fixtures/rpc/tag_remove.json, docs/work/architecture/protocol-v1.md (tag_ids order, tag.remove row) — stats.rs, writes.rs, tag_remove.json and protocol-v1.md also carry checkpoint-1 hunks

## 1b — goal scope labels

Every `GoalView` reads its task title, epic name or project name from the stored
scope row. All-work goals serialize `scope_label: null`. The RPC regression
renames a task before removal and checks its final title and the removed epic's
name in ended goals. Four fixtures gained only `scope_label`.

222 workspace tests passed. The eight Unix-socket tests listed under checkpoint
0b were filtered out. Strict Clippy, formatting and diff checks passed.

CHECKPOINT 1b: Add stored scope labels to goal views, including ended goals — files: crates/salah-domain/src/goal.rs, crates/salah-store/src/goals.rs, crates/salah-core/src/goals.rs, crates/salah-cli/tests/rpc/goals.rs, crates/salah-cli/tests/fixtures/rpc/{goal_add,goal_edit_successor,goals_list,status_goals}.json, docs/work/architecture/protocol-v1.md, docs/work/tracking-ui/c8-progress.md
