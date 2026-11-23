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
