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
