# TS progress

## stats.series (Trends line chart)

`stats.series` reuses the `stats.table` aggregation (`table::cells`, columns, key functions, `dated`, `weeks`, `days`, now `pub(super)`) and the shared session filter; no SQL change was needed (`report_sessions`). Interval `day` (default) or `week` (Monday first, first week clipped). Null values for empty periods except minutes and sessions. Split by project/tag/item: top 8 by minutes plus `other` (each session once). Empty `measures` or a range over 730 days is validation; an unknown measure is not_found. CLI: `salah stats series --measure focus --split item ...`.

Verification: `cargo test --offline --workspace` (socket tests included), strict Clippy, `cargo fmt --check` and `git diff --check` all pass. No existing fixture changed.

### Files

- `crates/salah-core/src/stats/series.rs`, `crates/salah-core/src/stats/series/tests.rs` (new)
- `crates/salah-core/src/stats.rs`, `crates/salah-core/src/stats/table.rs`, `crates/salah-core/src/stats/table/tests.rs`
- `crates/salah-protocol/src/{dispatcher,methods,reads}.rs`
- `crates/salah-cli/src/reports.rs`, `crates/salah-cli/tests/{daemon.rs,tracking_cli.rs,rpc/reports.rs}`
- `crates/salah-cli/tests/fixtures/rpc/{stats_series_day,stats_series_split_item}.json` (new)
- `docs/work/architecture/protocol-v1.md`, `docs/work/tracking-ui/ts-progress.md`

CHECKPOINT TS: Add stats.series for the Trends line chart, sharing stats.table aggregation
