# C1 brief (Codex): backend review and hardening for tracking

You are worker C1 for work item `tracking-ui` in /home/saidburxon/rust/salah (branch
`architecture`). The orchestrator is the Claude session `salah-orch`. Two Claude workers are
editing `app/**` and `plugin/**` right now: do not touch those. You own `crates/**` and
`docs/work/architecture/contract-v1.md`, plus your progress file
`docs/work/tracking-ui/c1-progress.md`.

Read: `docs/AGENTS.md`, `docs/archive/tracking-model/plan.md` and `interface.md` (the contract
and its "Review decisions"), `docs/archive/tracking-model/state.md` (known gaps),
`docs/work/tracking-ui/plan.md` (what the user hit), `docs/work/architecture/protocol-v1.md`.

## Tasks, in order

1. **Review** the tracking backend (salah-domain, salah-store, salah-core, salah-protocol,
   salah-cli) for correctness bugs against the contract: session validation, required/optional
   metrics, archived metrics, timer clamp, day snapshots and freezing, fulfilment and
   unplanned minutes, stats aggregation and weighting, op_id replay, `changed` notifications
   (does every session/project/tag/metric mutation notify with the right entity names so both
   frontends refresh?). Write findings with file:line and a failing scenario in your progress
   file. Fix every confirmed bug with a regression test.
2. **Missing-metrics message:** the CLI/RPC message lists custom metrics by id; keep
   `data.missing` as ids but make the message use metric names.
3. **Stale running timer:** a timer left running past its day's Sleep boundary should not stay
   "running" forever. Add `stale: true` on `SessionView` when `now` is past the clamp point,
   and make `session.stop` on a stale timer use the clamped minutes (already the rule). Update
   fixtures (`UPDATE_FIXTURES=1 cargo test --offline --test rpc`) and protocol-v1.md.
4. **Snapshot catch-up cost:** the first item write after a long gap can write up to 366 day
   snapshots. Make sure it happens in one transaction and is bounded/fast (measure with a test:
   400-day gap completes under ~1 s in debug), or materialise lazily only the days that are read.
5. **Docs:** mark `docs/work/architecture/contract-v1.md` as superseded by protocol-v1.md and
   the tracking-model interface (short header note, do not rewrite history), or update it if
   small.

Do not change the existing protocol shapes in a breaking way (frontends depend on the fixtures);
additive fields only. Keep files under ~800 lines; follow the existing module layout.

## Checks

`cargo test --offline --workspace` (socket tests will be skipped or fail in your sandbox; say
which), `cargo clippy --offline --all-targets -- -D warnings`, `cargo fmt --check`,
`git diff --check`.

## Checkpoint protocol

Your sandbox cannot write `.git` or create Unix sockets. Never run `git commit`. At each green
checkpoint append to your progress file a line `CHECKPOINT <task>: <one-line commit message>`
listing the files, then END your turn. The orchestrator runs the socket tests, reviews, commits,
and resumes you with the next task. Do not start the next task before you are resumed.
