# R5 brief (Codex): read-only review of the fixes since R4

You are reviewer R5 for `tracking-ui` in /home/saidburxon/rust/salah (branch `architecture`).
Other workers are editing `app/**` and `plugin/**` right now: do NOT edit any file except your
findings file `docs/work/tracking-ui/r5-findings.md`. Never run `git` write commands. Review the
COMMITTED changes since `305549b` (the R4 findings commit) (`git log --oneline 305549b..HEAD`, `git show <sha>`,
`git show HEAD:<path>`), not the working tree. You may run `cargo test --offline` (read-only use).

Scope: migration 005 + item_tags, tags on tasks (0c), block_minutes in stats.get (0b), goals
(core/store/protocol/CLI, scope_label), insights.get, review.get/save, the read-time rule "a session
follows its task's current project and epic" (data-model.md last section), the CLI goal/insights/
review/open --page commands, and the committed app pages/backends for goals, insights, review,
--page, tags, plus the widget goal chips, review footer and creation-sheet tags.

Read first: plan.md ("User decisions", "Phase 4", the addenda incl. "tags on tasks", "sessions
across prayer blocks", "user feedback 2026-11-24"), data-model.md (incl. the source-of-truth
section), docs/work/architecture/protocol-v1.md, fixtures in crates/salah-cli/tests/fixtures/rpc/.

Look for correctness bugs only, especially ones that corrupt or mislabel the user's data:
- any read path that still groups/filters item sessions by the stored sessions.project_id/epic_id
  (grep every SQL and Rust path), or double counts a session (tags, blocks, goals, review);
- goal progress wrong at period edges (Sleep boundary, Monday, mid-period edits, successor chain,
  at_most with no data, metric rollups and minute weighting, ticks);
- insights/review numbers disagreeing with stats.table for the same filter and range;
- review week validation, replay (op_id), empty note removal, changed notifications;
- validation gaps (non-finite numbers, dates, ranges > 730 days, unknown ids, removed tags);
- app/widget: request params differing from protocol/fixtures, stale state after `changed`,
  stale responses overwriting newer filters, Text.PlainText missing, wrong field names, the widget
  putting non-Item children into Omarchy Ui types (drops the whole widget in the live shell);
- SQL that is O(n²) at a few thousand sessions.

Append each finding as soon as it is confirmed (you may be cut off):
`- [high|medium|low] file:line — defect — concrete failing scenario — suggested fix`.
Write `DONE` as the last line. No praise, no style nits.

R5 focus: verify each R4 finding in r4-findings.md is really fixed (not just tested), and look for
regressions the fixes introduced: the Routines page, resources staged in the entry form and widget
creation sheet (op_id reuse, partial failures), link normalisation, Review drafts per week, task
edit versions, goal successor chains and dated membership, deterministic averages, CLI resolvers.
Nobody else is editing right now, but still edit nothing except r5-findings.md.
