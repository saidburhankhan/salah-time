# State: tracking-model

## Repository

- Branch: `architecture`, clean at 009249b; the architecture work item is finished (c673a99).

## Now

Backend complete and reviewed: S1 7d1299e, S2 bf4de55, S3 241326d c1753ec, S4 4b1c5b7 dbbf021,
review fixes 496da37 (decisions in interface.md "Review decisions"). 125 workspace tests incl.
daemon sockets, 0 ignored; strict Clippy clean. Schema v1 was changed in place (no user data;
an older DB is renamed aside).

Frontend (W2, Codex) has uncommitted, unaccepted work in app/** and plugin/**; its first attempt
deleted 43/48 app tests and was rejected.

Limits at 22:57: Claude 5h 62% (resets 03:10), 7d 32%. Codex 5h exhausted (resets 00:39).

## Next

- 00:41 (systemd timer `salah-resume-w2`): Codex resumes W2 with `w2-resume.txt` (restore and
  adapt tests, switch to the real fixtures, fix the app and widget to the real shapes).
- 00:44 (cron in salah-orch): confirm W2 is running and re-arm the Monitor; verify and commit
  checkpoints (explicit paths only).
- Then S6: `./scripts/test`, clippy, `./scripts/install`, `salah status --json`, daemon and
  journal checks; archive; `handoff-telegram.sh`.

## Interfaces

- `interface.md` is the contract between W1 (crates/**) and W2 (app/**, plugin/**).

## Risks and blockers

- Concurrent architecture refactor touches the same domain/planner files.

## Verification

- None yet (documentation only).
