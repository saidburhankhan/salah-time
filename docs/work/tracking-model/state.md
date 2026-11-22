# State: tracking-model

## Repository

- Branch: `architecture` (other agents have uncommitted work in
  `salah-domain/src/item.rs`, `salah-core/src/planner/`, `salah-protocol/`)
- Dirty files from this work item: `docs/work/tracking-model/`, `docs/START.md`

## Now

S0 is locked (`interface.md`). No code has been changed. The architecture lead (Claude
session `06b42b65`, interactive, its name is the raw "@docs/START.md Take role as lead…"
prompt) is still busy finishing its step; a DeepSeek worker has uncommitted
`app/tests/*` changes. Codex is at 100% of its 5h window and resets at 19:37 local
(rate_limits `resets_at` 1795358253).

Orchestrator survival: the `salah-revive.service` user unit runs `revive.sh` and resumes
this session (`1e3cb88a…`) in the background after a limit cutoff (log:
`~/.cache/salah-agents/revive.log`). The Claude status line writes
`~/.cache/salah-agents/claude-status.json` (5h usage and reset time).

## Next

1. `salah-lead` (W1) committed c673a99, freed app/** and plugin/**, and has the W1 brief;
   it waits for the user's OK before rewriting crates/**.
2. At 19:39 (cron in `salah-orch`): dispatch `w2-brief.md` to Codex.

## Interfaces

- Implementation replaces `Item`/`Entry`/`entries` wholesale; it must start
  after the `architecture` work item lands to avoid conflicting edits.

## Risks and blockers

- Concurrent architecture refactor touches the same domain/planner files.

## Verification

- None yet (documentation only).
