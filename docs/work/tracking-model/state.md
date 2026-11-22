# State: tracking-model

## Repository

- Branch: `architecture`, clean at 009249b; the architecture work item is finished (c673a99).

## Now

Planning is committed (27f7020). `salah-lead` has ended, so W1 (backend) goes to Codex.
Limits at 19:10: Claude 5h 75% (resets 22:10), 7d 22%; Codex 5h 100% (resets 19:37), 7d 16%.
The orchestrator `salah-orch` stays lean so it outlives the workers; `salah-revive.service`
resumes it after a limit cutoff.

## Next

- W1 is running as `salah-codex-w1` (Codex, started 19:39; Codex 5h window resets 00:39). Log: `~/.cache/salah-agents/w1.log`.
- At :17/:57 each hour (recurring cron): check limits and progress; dispatch W2 once W1
  has committed S2 and Codex 5h usage is below 60%; re-dispatch a dead worker; integrate
  and verify when both are done.

## Interfaces

- `interface.md` is the contract between W1 (crates/**) and W2 (app/**, plugin/**).

## Risks and blockers

- Concurrent architecture refactor touches the same domain/planner files.

## Verification

- None yet (documentation only).
