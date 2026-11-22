# State: tracking-model

## Repository

- Branch: `architecture`, clean at 009249b; the architecture work item is finished (c673a99).

## Now

Planning is committed (27f7020). `salah-lead` has ended, so W1 (backend) goes to Codex.
Limits at 19:10: Claude 5h 75% (resets 22:10), 7d 22%; Codex 5h 100% (resets 19:37), 7d 16%.
The orchestrator `salah-orch` stays lean so it outlives the workers; `salah-revive.service`
resumes it after a limit cutoff.

## Next

- W1 (`salah-codex-w1`) and W2 (`salah-codex-w2`) both run on Codex since 19:39/19:5x
  (Codex 5h window resets 00:39). Logs in `~/.cache/salah-agents/`. The orchestrator
  watches commits, unit exits and Codex errors live (Monitor), re-dispatches dead
  workers, and reviews each commit against `interface.md`.
- 22:13 (cron): Claude has reset; add Claude subagents next to Codex where ownership allows.
- When both slices are integrated and verified: archive this work item, then run
  `handoff-telegram.sh` (the user asked for this) to resume the telegram-vim-rust lead on its
  task list 1–4 (order 4, 1, 2, 3).

## Interfaces

- `interface.md` is the contract between W1 (crates/**) and W2 (app/**, plugin/**).

## Risks and blockers

- Concurrent architecture refactor touches the same domain/planner files.

## Verification

- None yet (documentation only).
