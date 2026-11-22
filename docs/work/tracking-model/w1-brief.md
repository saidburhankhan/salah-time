# W1 brief (backend, S1–S4)

The previous architecture lead has finished; crates/** is free. Architecture context, if needed: docs/work/architecture/{state.md,protocol-v1.md}.

You are worker W1 (Codex) for work item `tracking-model` in /home/saidburxon/rust/salah, branch `architecture` (HEAD c673a99, clean). The orchestrator is the Claude session `salah-orch`.

Read, in order: docs/AGENTS.md, docs/work/tracking-model/plan.md, docs/work/tracking-model/interface.md. interface.md is the locked contract; do not change it — put requested changes in your report.

Outcome: slices S1–S4 — replace Item/Entry/entries with the tracking model (projects, tags, metrics, project_metrics, items with project_id/planned_minutes, sessions + session_values, day_blocks, day_plan) across salah-domain, salah-store (single clean v1 schema, FKs on, CHECKs, indexes; old DB renamed aside, there is no user data), salah-core (planner, session logging with metric validation, fulfilment, snapshots, stats per interface), salah-protocol and the daemon/CLI (methods per interface; regenerate crates/salah-cli/tests/fixtures/rpc/ and add fixtures for every new method), and update docs/work/architecture/protocol-v1.md.

You own: crates/**, docs/work/architecture/protocol-v1.md. Do NOT edit app/**, plugin/** (owned by the frontend worker W2), or docs/work/tracking-model/** and docs/START.md (orchestrator).

Work in order S1 → S2 → S3 → S4. Commit at each green checkpoint (./scripts/test + cargo clippy --offline --all-targets -- -D warnings + git diff --check), on branch architecture. The app/widget may stop working against the new protocol between commits; that is expected — do not patch them.

Usage limits: work economically and keep reads targeted. If you may be cut off, the progress file is how the next run resumes. Keep a short progress file at docs/work/tracking-model/w1-progress.md (you may write that one file) with: last commit, current slice, next step — update it at every commit, so work can resume if you are cut off.

When S4 is committed, or if you are blocked, finish with a final message (captured for the orchestrator) and the same content in w1-progress.md: commits, verification output, deviations from interface.md, remaining risks.

## Checkpoint protocol (overrides any "commit" instruction above)

Your sandbox cannot write `.git` or create Unix sockets. So: never run `git commit`.
At each green checkpoint (all checks pass except tests that need Unix sockets),
append to your progress file a line `CHECKPOINT <slice>: <one-line commit message>`
listing the files in that checkpoint, then END your turn (exit). The orchestrator runs the
socket tests, reviews, commits, and resumes you with the next step. Do not start the next slice
before you are resumed.
