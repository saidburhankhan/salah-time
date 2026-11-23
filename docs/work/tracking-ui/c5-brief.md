# C5 brief (Codex): backend for resources and backlog

You are worker C5 for work item `tracking-ui` phase 3 in /home/saidburxon/rust/salah (branch
`architecture`). You own `crates/**` and `docs/work/architecture/protocol-v1.md` and `docs/work/tracking-ui/c5-progress.md`. Other workers may edit
other directories concurrently; never touch them.

Read: `docs/AGENTS.md`, `docs/work/tracking-ui/data-model.md` (locked storage and protocol rules; it wins over plan.md on storage), `docs/work/tracking-ui/plan.md` (the "User decisions", "Phase 2" and
"Phase 3" sections; phase 3's contract is locked, implement it exactly), the previous progress files
in `docs/work/tracking-ui/` for how this area was built and verified, and the code you own.

Order (user priority 2026-11-23: epics are the most important part of phase 3): (0), then (2b)
epics, then (1) backlog, then (2) resources, then (3)-(4).

Tasks: (0) migration 004 exactly as data-model.md specifies (items rebuild, epics, resources,
session/day_plan epic_id) with its migration test; CHECKPOINT after this alone for the independent
review. (1) nullable day/block for one-time items with a CHECK, backlog semantics in day plans,
snapshots, stats and fulfilment (backlog items never count), `item.add backlog`, `item.schedule`,
`item.backlog`, `backlog.list`, `item.move` in the backlog; (2) resources table and methods with
validation, day-row resource counts, `status.get.reminders`, `changed` entity `resources`;
(2b) epics as plan.md "Epics" specifies: table, methods, item/session `epic_id`, epic resources,
the stats/table/sessions.list filters and grouping, its own CHECKPOINT; (3) CLI `salah epic ...`, `salah backlog [list|add|schedule]`, `salah resource [list|add|edit|remove]`; (4) fixtures
named in plan.md and protocol-v1.md. Schema changes are additive and migrate an existing database
in place (the user has live data now: never drop or rewrite it). One CHECKPOINT after (2b), one after (1), one after
(2)-(4). Checks: `cargo test --offline --workspace` (socket tests fail in your sandbox; say which),
`cargo clippy --offline --all-targets -- -D warnings`, `cargo fmt --check`, `git diff --check`.

Also: `stats.get`/`stats.table`/`sessions.list` already accept `"none"` and the duration/metric filters (8c7adfa); add `epic_id` (incl. `"none"`) to that shared filter.

Epics are optional everywhere and existing projects, tasks, routines, sessions and fixtures must
keep working unchanged; read plan.md "Optional everywhere" and "Statistics show the gaps" first.

## Checkpoint protocol

Your sandbox cannot write `.git` or create Unix sockets. Never run `git commit`. At each green
checkpoint (all checks pass except tests that need Unix sockets) append to your progress file a
line `CHECKPOINT <slice>: <one-line commit message>` listing the files, then END your turn. The
orchestrator (Claude session `salah-orch`) runs the socket tests, reviews, commits, and resumes you.
Do not edit files outside your ownership. Never touch ~/.local/share/salah or the live daemon;
do not run ./scripts/install.
