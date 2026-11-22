# State: tracking-model

## Repository

- Branch: `architecture`, clean at 009249b; the architecture work item is finished (c673a99).

## Now

Committed: S1 7d1299e, S2 bf4de55, S3 core 241326d + c1753ec (sessions, metric validation,
snapshots, fulfilment, day/metric/project/tag/block stats). All workspace suites including
daemon sockets + strict Clippy green at each commit.

Uncommitted, not green-checked: W1's partial S4 protocol wiring (`crates/salah-protocol`,
RPC fixtures, Cargo.lock) and W2's frontend work (`app/**`, `plugin/**`). W2's first attempt
was rejected: it deleted 43 of 48 app tests; it was told to restore and adapt them.

Both Codex workers stopped at 20:15 on the Codex usage limit (resets 00:39).
Codex workers run sandboxed (no .git writes, no sockets): they declare CHECKPOINT in
their progress file and the orchestrator verifies and commits.

## Next

- 22:13 (cron): Claude resets at 22:10. Start a Claude worker on W1's remaining S4
  (protocol wiring, legacy removal, fixtures) from w1-progress.md, so the critical path
  doesn't wait for Codex.
- 00:41 (systemd timer `salah-resume-w2`): Codex resumes W2 (test restoration).
  W1 Codex is resumed by the orchestrator only if the Claude worker hasn't finished S4.
- Then: S6 integration (full checks, install, live daemon/logs), archive, and
  `handoff-telegram.sh`.

## Interfaces

- `interface.md` is the contract between W1 (crates/**) and W2 (app/**, plugin/**).

## Risks and blockers

- Concurrent architecture refactor touches the same domain/planner files.

## Verification

- None yet (documentation only).
