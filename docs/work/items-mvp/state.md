# State: items-mvp

## Repository

- Branch: `items-mvp` (from `sleep-boundary`)
- Dirty files: none before this work

## Now

Plan and data contract written. No code yet.

## Next

S1 (Codex) and S2 (lead) in parallel.

## Interfaces

- `status --json` keeps every existing field; additions are additive.
- Workers own only the files listed for their slice in `plan.md`.

## Risks and blockers

- DeepSeek headless needs `DEEPSEEK_API_KEY` exported from the dsh credential store.

## Verification

- Not run yet.
