# C2 brief (Codex): backend for stats table, filters, Save & Done

You are worker C2 for work item `tracking-ui` phase 2 in /home/saidburxon/rust/salah (branch
`architecture`). You own `crates/**`, `docs/work/architecture/protocol-v1.md` and
`docs/work/tracking-ui/c2-progress.md`. Worker C3 edits `plugin/**` concurrently; C4 will edit
`app/**` after your first checkpoint.

Read: `docs/AGENTS.md`, `docs/work/tracking-ui/data-model.md` (locked storage and protocol rules; it wins over plan.md on storage), `docs/work/tracking-ui/plan.md` (especially "Phase 2": its contract
additions are locked; implement them exactly), `docs/archive/tracking-model/interface.md`,
`docs/work/architecture/protocol-v1.md`, `docs/work/tracking-ui/c1-progress.md` (previous
backend worker's notes), and the existing stats code in `crates/salah-core/src/stats.rs`.

Tasks:
0. The migration chain and migration 003 exactly as data-model.md specifies (with the backup
   copy and the per-step migration tests). CHECKPOINT after this alone: it gets an independent review.
1. `stats.get`: `item_id?` filter; any from/to range up to 730 days (validation error beyond).
2. `session.stop` `minutes?` override (plan.md) and `done?: bool` (stop with values + tick in one write, replay-safe like the
   existing tick-stops-timer path).
3. `stats.table` as specified, computed in core/SQL, with tests for each group, weighting,
   tag double counting, empty weeks, filters, and archived metrics (archived metrics with data in
   range still appear as columns; without data they are omitted).
2b. Built-in `distractions` metric (migration adds it to existing projects' asked metrics),
   `session.bump`, and the derived per-hour column in `stats.table`.
2c. Per-task `metric_ids` with the effective `metrics` list on day rows and in status.prompt.
2d. `session.note` (append a timestamped line to the running session's note).
3b. `sessions.list` (Notes page query) as specified, filtered in SQL, with tests for every filter
   and the cursor.
4. CLI: `salah sessions list --q ... --min-focus ...` style flags, `salah stats table --group week ...` printing the RPC JSON; `salah session stop --done`.
5. Fixtures (`UPDATE_FIXTURES=1 cargo test --offline --test rpc`) named in plan.md, and
   protocol-v1.md.

Checkpoint after 1+2 (so the frontends can start), then after 3-5.
Checks: `cargo test --offline --workspace` (socket tests fail in your sandbox; say which),
`cargo clippy --offline --all-targets -- -D warnings`, `cargo fmt --check`, `git diff --check`.
Keep files under ~800 lines; follow the module layout.

## Checkpoint protocol

Your sandbox cannot write `.git` or create Unix sockets. Never run `git commit`. At each green
checkpoint (all checks pass except tests that need Unix sockets) append to your progress file a
line `CHECKPOINT <slice>: <one-line commit message>` listing the files, then END your turn. The
orchestrator (Claude session `salah-orch`) runs the socket tests, reviews, commits, and resumes you.
Do not edit files outside your ownership. Never touch ~/.local/share/salah or the live daemon;
do not run ./scripts/install.
