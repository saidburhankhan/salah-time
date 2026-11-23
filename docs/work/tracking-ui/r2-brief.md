# R2 brief (Codex): read-only review of the epics and backlog backend

You are reviewer R2 for `tracking-ui` in /home/saidburxon/rust/salah (branch `architecture`).
Other workers are editing `crates/**` and `app/**` right now: do NOT edit any file except your
findings file `docs/work/tracking-ui/r2-findings.md`. Never run `git` write commands. Review the
commits with `git show 39f8e32` and `git show c935922` (and `git show <sha>:<path>`), not the
working tree.

Read `docs/work/tracking-ui/data-model.md` (004 and its Rules) and plan.md "Phase 3" incl. "Epics",
"Optional everywhere" and "Statistics show the gaps".

Look for correctness bugs only:
- data loss or corruption on the user's live database;
- an epic from another project accepted on an item;
- a session's `project_id`/`epic_id` rewritten after it was written;
- finished/removed epic rules;
- backlog items leaking into day_plan, snapshots, capacity or fulfilment;
- `item.schedule`/`item.backlog` on routines or on frozen past days;
- writes bypassing op_id replay, `changes` or `version`;
- `"none"` filters giving wrong totals;
- No project / No epic rows double-counting or missing work.

Append each finding as soon as it is confirmed (you may be cut off):
`- [high|medium|low] file:line — defect — concrete failing scenario — suggested fix`.
Write `DONE` as the last line. No praise, no style nits.
