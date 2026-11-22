# State: tracking-model

## Repository

- Branch: `architecture` (other agents have uncommitted work in
  `salah-domain/src/item.rs`, `salah-core/src/planner/`, `salah-protocol/`)
- Dirty files from this work item: `docs/work/tracking-model/`, `docs/START.md`

## Now

Planning is committed (27f7020). `salah-lead` has ended, so W1 (backend) goes to Codex.
Limits at 19:10: Claude 5h 75% (resets 22:10), 7d 22%; Codex 5h 100% (resets 19:37), 7d 16%.
The orchestrator `salah-orch` stays lean so it outlives the workers; `salah-revive.service`
resumes it after a limit cutoff.

## Next

- 19:39 (one-shot cron): `dispatch-codex.sh w1-brief.md w1`.
- At :17/:57 each hour (recurring cron): check limits and progress; dispatch W2 once W1
  has committed S2 and Codex 5h usage is below 60%; re-dispatch a dead worker; integrate
  and verify when both are done.

## Interfaces

- Implementation replaces `Item`/`Entry`/`entries` wholesale; it must start
  after the `architecture` work item lands to avoid conflicting edits.

## Risks and blockers

- Concurrent architecture refactor touches the same domain/planner files.

## Verification

- None yet (documentation only).
