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

## 2 — insights

Week and weekday aggregates run in SQLite against the shared report scope. Local start hours, split block minutes and planned-block comparisons use each session's planning-day snapshot. Ranked sessions include the same references and notes as the Notes page. The RPC regressions cover cross-boundary allocation, local starts, filters, ticks, empty ranges, tie-breaking and the five-session limit.

224 workspace tests passed with the eight socket tests listed under 0b skipped. Strict Clippy, formatting and diff checks passed.

CHECKPOINT 2: Add session insights and planned versus actual block reports — files: crates/salah-core/src/{insights,lib,sessions/list}.rs, crates/salah-store/src/reports.rs, crates/salah-protocol/src/{dispatcher,reads}.rs, crates/salah-cli/tests/{daemon/methods,rpc,rpc/insights}.rs, crates/salah-cli/tests/fixtures/rpc/insights.json, docs/work/tracking-ui/c8-progress.md

## 3 — weekly review

The summary includes met daily and weekly periods, current count of live backlog tasks without a completion tick, noted sessions (including ticks), recorded minutes by assignment, and timed or logged work for the weekly distraction rates. Reflection saves are replayable and reject invalid Mondays and nonblank notes longer than 5000 characters. An empty reflection soft-deletes the row. Existing mutation fixtures gained only the empty reviews array.

226 workspace tests passed with the eight socket tests listed under 0b skipped. Strict Clippy, formatting and diff checks passed.

CHECKPOINT 3: Add weekly summaries and replayable reflection saves — files: crates/salah-cli/tests/daemon/methods.rs, crates/salah-cli/tests/fixtures/rpc/epic_add.json, crates/salah-cli/tests/fixtures/rpc/epic_edit_finish.json, crates/salah-cli/tests/fixtures/rpc/goal_add.json, crates/salah-cli/tests/fixtures/rpc/goal_edit_successor.json, crates/salah-cli/tests/fixtures/rpc/item_add.json, crates/salah-cli/tests/fixtures/rpc/item_add_backlog.json, crates/salah-cli/tests/fixtures/rpc/item_add_quick.json, crates/salah-cli/tests/fixtures/rpc/item_add_quick_tomorrow.json, crates/salah-cli/tests/fixtures/rpc/item_add_tags.json, crates/salah-cli/tests/fixtures/rpc/item_backlog.json, crates/salah-cli/tests/fixtures/rpc/item_edit.json, crates/salah-cli/tests/fixtures/rpc/item_edit_metrics.json, crates/salah-cli/tests/fixtures/rpc/item_move.json, crates/salah-cli/tests/fixtures/rpc/item_remove.json, crates/salah-cli/tests/fixtures/rpc/item_schedule.json, crates/salah-cli/tests/fixtures/rpc/metric_add.json, crates/salah-cli/tests/fixtures/rpc/metric_edit.json, crates/salah-cli/tests/fixtures/rpc/prayer_mark.json, crates/salah-cli/tests/fixtures/rpc/project_add.json, crates/salah-cli/tests/fixtures/rpc/project_edit.json, crates/salah-cli/tests/fixtures/rpc/resource_add_link.json, crates/salah-cli/tests/fixtures/rpc/resource_add_reminder.json, crates/salah-cli/tests/fixtures/rpc/resource_edit_done.json, crates/salah-cli/tests/fixtures/rpc/review_get.json, crates/salah-cli/tests/fixtures/rpc/review_save.json, crates/salah-cli/tests/fixtures/rpc/session_bump.json, crates/salah-cli/tests/fixtures/rpc/session_edit.json, crates/salah-cli/tests/fixtures/rpc/session_edit_remove_note.json, crates/salah-cli/tests/fixtures/rpc/session_log.json, crates/salah-cli/tests/fixtures/rpc/session_log_epic.json, crates/salah-cli/tests/fixtures/rpc/session_log_tick.json, crates/salah-cli/tests/fixtures/rpc/session_log_tick_stops_timer.json, crates/salah-cli/tests/fixtures/rpc/session_note.json, crates/salah-cli/tests/fixtures/rpc/session_remove.json, crates/salah-cli/tests/fixtures/rpc/session_start.json, crates/salah-cli/tests/fixtures/rpc/session_stop.json, crates/salah-cli/tests/fixtures/rpc/session_stop_done.json, crates/salah-cli/tests/fixtures/rpc/session_stop_minutes.json, crates/salah-cli/tests/fixtures/rpc/tag_add.json, crates/salah-cli/tests/fixtures/rpc/tag_edit.json, crates/salah-cli/tests/fixtures/rpc/tag_remove.json, crates/salah-cli/tests/rpc.rs, crates/salah-cli/tests/rpc/reviews.rs, crates/salah-core/src/goals.rs, crates/salah-core/src/lib.rs, crates/salah-core/src/reviews.rs, crates/salah-domain/src/lib.rs, crates/salah-domain/src/review.rs, crates/salah-protocol/src/dispatcher.rs, crates/salah-protocol/src/lib.rs, crates/salah-protocol/src/message.rs, crates/salah-protocol/src/methods.rs, crates/salah-protocol/src/reviews.rs, crates/salah-protocol/tests/dispatch.rs, crates/salah-store/src/goals.rs, crates/salah-store/src/lib.rs, crates/salah-store/src/reviews.rs, docs/work/tracking-ui/c8-progress.md

## 4 — CLI

`salah goal [list|add|edit|end|remove|history]`, `salah insights` (same range and filter flags as stats) and `salah review [show|save]` call the same protocol methods as RPC, so each supports `--json`; human output is compact plain text (goal lines, insights sections by block/weekday/start hour/week/plan plus best and hardest sessions, review summary with the reflection). `review show` and `review save <note>` default `--week` to the Monday of the last full week (computed from `day.get`'s tracking day, pure `last_full_week`); an empty note clears the reflection. `salah open --page <today|backlog|routines|statistics|insights|goals|review|settings>` runs `salah-app --page <name>`; no flag passes no argument; unknown names fail listing the valid ones (pure `app_args`). The CLI is not documented in protocol-v1.md, so help text only.

Review refinement finished: `open_backlog_count` counts live backlog tasks with no completion tick (store query plus `reviews.rs` RPC test and two fixture edits: count 1 → 0 after the tick, ids shifted by one).

Tests: 4 new `tracking_cli.rs` tests (goals lifecycle, insights, review default week/save/clear, open rejection), unit tests for `last_full_week` and `app_args`. 242 workspace tests passed, 0 failed (socket tests included). fmt, clippy -D warnings and `git diff --check` clean.

CHECKPOINT 4: CLI goal, insights and review commands, review open-backlog refinement, `salah open --page` — files: crates/salah-cli/src/main.rs, crates/salah-cli/src/render.rs, crates/salah-cli/src/reports.rs, crates/salah-cli/src/goals.rs, crates/salah-cli/src/reviews.rs, crates/salah-cli/tests/tracking_cli.rs, crates/salah-cli/tests/rpc/reviews.rs, crates/salah-cli/tests/fixtures/rpc/review_get.json, crates/salah-cli/tests/fixtures/rpc/review_save.json, crates/salah-store/src/reviews.rs, docs/work/tracking-ui/c8-progress.md

## 5 — a session follows its task's current project and epic

One store query resolves task assignments for session reports, goal samples, epic totals and the epic removal check. The same query supplies IDs for running timers, mutations and sessions embedded in day views. Item-less logs keep their recorded project and epic. Backlog membership and epic task counts already use the current item, and completion ticks are tied to the item ID.

The new RPC fixture `stats_epic_reassigned.json` shows Bayyinah with 10 sessions and 30 minutes, all logged before the epic existed. The test starts six sessions' worth of work without a project and logs the other four in a different project. Both sets follow the task. Item-less project and epic logs stay in their original groups. The regression compares stored session rows before and after the moves.

Existing JSON fixtures retained all their values. Three older test expectations changed with the rule. Removing a task's assignment makes its open timer's project null. The previous project's filtered minutes drop from 15 to 5 after a task moves, leaving just the bare project log. A historical epic retains its frozen 30 planned minutes, while its 25 worked minutes and two session rows follow the task out of that epic.

235 workspace tests passed. The eight Unix-socket tests named under checkpoint 0b were filtered out. Strict Clippy, formatting and diff checks passed.

CHECKPOINT 5: Resolve session project and epic through the current task across reports, goals and session views — files: crates/salah-store/src/{reports,goals,epics}.rs, crates/salah-core/src/{sessions,sessions/list,stats/table,status}.rs, crates/salah-protocol/tests/dispatch.rs, crates/salah-cli/tests/{rpc,rpc/assignments,rpc/epics,rpc/goals,rpc/phase2}.rs, crates/salah-cli/tests/fixtures/rpc/stats_epic_reassigned.json, docs/work/architecture/protocol-v1.md, docs/work/tracking-ui/c8-progress.md

## 6 — weekly review, goal precision and CLI notes

Weekly review uses the same distraction numerator and recorded-minute denominator as the Stats table. Insights keeps its duration filter. The only changed value in an existing fixture is `review_get.json`'s weekly rate, from 2 to 22 per hour (1 logged distraction plus 10 on a tick over 30 minutes). All other fixture changes are additive counts and preview flags.

Review aggregates minutes by assignment in SQL and fetches a newest-first preview of up to 32 noted sessions, with up to 12 recent timestamped entries per session. The preview has a 384 KiB encoded budget. The project, epic and met-goal lists have 128 KiB each and report their full counts. A regression exercises 600 four-byte-character notes on one session, then 40 noted sessions, and checks that the serialized response fits the transport limit with a full-length reflection. Human Review and Insights print each entry's timestamp and text.

Average goal comparisons retain low-order terms in the weighted numerator and compare it with target times total weight before division. The reported equality case passes in both directions across status, history and review. Regression tests also cover very large finite means and adjacent unequal values. EXPLAIN QUERY PLAN on all four scopes confirms a range search on sessions_by_day or sessions_by_item_day, followed by resolved scope lookups where needed and a (session_id, metric_id) primary-key probe.

240 workspace tests passed. The eight Unix-socket tests named under checkpoint 0b were filtered out. Strict Clippy, formatting and diff checks passed.

CHECKPOINT 6: Correct weekly review rates, bound review previews, stabilize and index metric goal progress, and render timestamped CLI notes — files: crates/salah-cli/src/render.rs, crates/salah-cli/tests/rpc/{goals,reviews}.rs, crates/salah-cli/tests/fixtures/rpc/review_get.json, crates/salah-core/src/{goals,reviews}.rs, crates/salah-domain/src/review.rs, crates/salah-store/src/{goals,reports,reviews}.rs, docs/work/architecture/protocol-v1.md, docs/work/tracking-ui/c8-progress.md

## 7 — historical report IDs, goal overflow and membership, unplanned work

Stats (summary, table and series), Insights and the Notes CLI list accept a removed task's full ID or unique prefix. Mutation commands retain live-only lookup. The end-to-end CLI regression checks all five reports and rejects an edit using either form of the removed ID.

A goal period whose sum exceeds the finite numeric range returns `error: "sum_overflow"`, `value: null` and `met: false`. The error is local to the affected progress or history row. Regression coverage includes list, history, status, review and ending the affected goal.

Fixed-date membership uses the ended version's existing `updated_at` closing instant and the requested date's ending Sleep boundary (frozen clock or applicable settings). A closure exactly at Sleep takes effect on the following tracking day. History and `include_ended` keep the original period bounds. The test repeats the same date queries on successive days, then changes the Sleep setting on a later day and checks the historical result again. This uses schema version 5.

Item-less work produces a single Unplanned row with null task and planned block, its filtered minutes, and no adherence percentage. The regression checks project and epic logs together, separately by epic filter, and against the block matrix. All existing golden fixtures passed without edits.

244 workspace tests passed. The eight socket tests named under checkpoint 0b were filtered out. Strict Clippy, formatting and diff checks passed.

CHECKPOINT 7: Resolve removed report tasks, report overflowing goal sums, fix dated goal membership and include item-less work in Insights adherence — files: crates/salah-cli/src/{local,render}.rs, crates/salah-cli/tests/{tracking_cli,tracking_cli/report_items,rpc/goals,rpc/insights}.rs, crates/salah-core/src/{day/mod,day/snapshot,goals,insights,items,status}.rs, crates/salah-domain/src/goal.rs, crates/salah-protocol/src/goals.rs, crates/salah-store/src/read.rs, docs/work/architecture/protocol-v1.md, docs/work/tracking-ui/c8-progress.md

## 8 — deterministic report averages

Stats and Insights block and hour rollups order samples by value and weight, sum weighted contributions, and divide once. Very large finite values use an ordered incremental mean if the numerator overflows. Goal averages continue to compare compensated numerators in value-and-weight order. The SQL shared by Insights and Review orders the focus and distraction contributions within each aggregate.

Exact fixture comparison replaces the nine-decimal rounding that masked the flake. The only fixture edits are last-digit changes in `stats_table_week.json`, `stats_table_project.json`, `stats_series_day.json`, `goals_list.json`, and `status_goals.json`. W20 focus consistently renders as `2.608695652173913`.

All 42 RPC tests, including the weekly stats fixture test, passed in ten consecutive runs. 244 workspace tests passed with the eight Unix-socket tests listed under checkpoint 0b filtered out. Strict Clippy, formatting, and diff checks passed.

CHECKPOINT 8: Make report averages deterministic and compare RPC fixtures exactly — files: crates/salah-core/src/{stats,insights,goals}.rs, crates/salah-store/src/reports.rs, crates/salah-cli/tests/rpc.rs, crates/salah-cli/tests/fixtures/rpc/{stats_table_week,stats_table_project,stats_series_day,goals_list,status_goals}.json, docs/work/tracking-ui/c8-progress.md

## 9 — adherence, note pagination, goal indexes, and distraction rates

Insights combines each task's minutes into one adherence row and tests each session against its historical planned block. A move from Fajr to Asr produces one 30-of-60-minute row. The matrix retains both historical cells. Best and Hardest each have a 192 KiB preview budget and expose the latest 12 timestamped entries per session with `notes_count` and `notes_more`. The regression verifies 300 four-byte-character notes fit in both ranked previews and that `session.notes` can fetch every entry in three pages. The Insights fixture gains preview fields and `planned_blocks` only.

Weekly `review.notes` searches all live main notes and timestamped entries in finished sessions. It returns matching session rows with matching entries and a cursor that can continue within a session. The search test retrieves entries omitted from the Review preview, excludes a deleted note and a note from the prior week, and checks Unicode matching and pagination. Desktop integration belongs to the app slice. The Review filter can call `review.notes` and the session editor can follow `session.notes` pages when `notes_more` is true.

Project and epic goals use disjoint branches for assigned tasks and direct logs on the existing schema. EXPLAIN QUERY PLAN verifies searches using `items_by_project` and the `sessions_by_item_day`, `sessions_by_project_day`, and `sessions_by_epic_day` indexes, followed by composite metric lookups. The RPC regression checks that moved-task and direct work contribute once to each relevant goal.

Stats, Review, and Insights use the same compensated sum and hourly rate for the same distraction samples. The fractional regression reports 9.75/hour across Stats, Review, and the Insights calendar and block groups.

249 workspace tests passed. Strict Clippy, formatting, and diff checks passed. The eight Unix-socket tests listed under checkpoint 0b were filtered out.

CHECKPOINT 9: Aggregate adherence per task, paginate notes, index assignment goal reads, and share distraction rates — files: crates/salah-cli/src/render.rs, crates/salah-cli/tests/{daemon/methods,rpc/goals,rpc/insights,rpc/reviews}.rs, crates/salah-cli/tests/fixtures/rpc/insights.json, crates/salah-core/src/{insights,reviews,sessions,sessions/notes,stats/table}.rs, crates/salah-domain/src/{lib,reports}.rs, crates/salah-protocol/src/{dispatcher,reads,reviews}.rs, crates/salah-store/src/{goals,lib,note_queries,reports,reviews}.rs, docs/work/architecture/protocol-v1.md, docs/work/tracking-ui/c8-progress.md

## 10 — routine activity

`routines.activity` groups finished duration logs and completion ticks by routine and tracking day in SQLite. The regression covers removed sessions and tasks, running timers, one-time and bare work, tick-only days, invalid ranges and limits, and a 502-row result paged through RPC and the CLI fallback. The new golden fixture is `routines_activity.json`.

251 workspace tests passed. Strict Clippy, formatting, and diff checks passed. The eight Unix-socket tests listed under checkpoint 0b were filtered out.

CHECKPOINT 10: Page daily routine minutes and completion ticks through RPC and CLI — files: crates/salah-cli/src/{main,reports}.rs, crates/salah-cli/tests/{daemon/methods,rpc,rpc/routines}.rs, crates/salah-cli/tests/fixtures/rpc/routines_activity.json, crates/salah-core/src/{lib,routines}.rs, crates/salah-domain/src/item.rs, crates/salah-protocol/src/{dispatcher,reads}.rs, crates/salah-store/src/reports.rs, docs/work/architecture/protocol-v1.md, docs/work/tracking-ui/c8-progress.md
