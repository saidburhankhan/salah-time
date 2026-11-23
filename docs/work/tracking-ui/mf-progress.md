# MF: metric and duration filters on Statistics

`stats.get` and `stats.table` accept `minutes_min?`, `minutes_max?`, `values?: {metric_id: {min?, max?}}`
with the `sessions.list` semantics, and echo them in `filter` (`minutes_min`/`minutes_max` null, `values` {}
when absent). Planned minutes, capacity and the fulfilment denominator are unaffected.

- Store: `SessionFilter` (reports.rs) holds the shared predicate; `SessionScope.measures` applies it to every report.
- Core: `stats/measures.rs` (`Measures`, `ValueRange`, validation). `Filter` flattens `Measures`. `sessions.list` builds
  the same `Measures` from its query (no copied predicate code).
- `stats.get` now reads its sessions through `report_sessions`, so it also accepts `"none"` for project/tag/item (C5 item);
  plan lines narrow the same way (`Scope` in stats.rs). Protocol: `ScopeParams` (range+filter), `StatsParams` adds measures.
- CLI: shared `MeasureArgs` flags (`--min-minutes`, `--min-focus`, `--min METRIC=V`, ...) on `salah stats`, `salah stats table`, `salah sessions list`.
- Tests: rpc `stats_get_and_table_narrow_sessions_by_duration_and_metric_values`; fixtures `stats_metric_filter`, `stats_table_metric_filter`.
  Existing fixtures only gained `minutes_min`, `minutes_max`, `values` in `filter`.
- Pre-existing flake fixed: `stats_table_week` mean differed in the last digit between runs (ties sorted by random id);
  `compare_fixture` now rounds floats to 9 places when comparing.
- Note: a tick has no minutes, so a duration bound never matches it.

Checks: cargo test --workspace (socket tests included), clippy -D warnings, fmt --check, git diff --check all pass.

CHECKPOINT MF: filters on stats.get/stats.table; files: crates/salah-store/src/reports.rs, crates/salah-core/src/stats.rs, crates/salah-core/src/stats/measures.rs, crates/salah-core/src/stats/table/tests.rs, crates/salah-core/src/sessions/list.rs, crates/salah-core/src/sessions/list/tests.rs, crates/salah-protocol/src/methods.rs, crates/salah-protocol/src/reads.rs, crates/salah-cli/src/reports.rs, crates/salah-cli/src/tracking.rs, crates/salah-cli/tests/rpc.rs, crates/salah-cli/tests/rpc/reports.rs, crates/salah-cli/tests/fixtures/rpc/{stats,stats_by_tag,stats_item_filter,stats_table_project,stats_table_week,stats_metric_filter,stats_table_metric_filter}.json, docs/work/architecture/protocol-v1.md, docs/work/tracking-ui/mf-progress.md
