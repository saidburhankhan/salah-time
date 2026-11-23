# R1 brief (Codex): read-only review of today's phase 2 commits

You are reviewer R1 for `tracking-ui` in /home/saidburxon/rust/salah (branch `architecture`).
Two Claude workers are editing `crates/**` and `app/**` right now: do NOT edit any file except
your findings file `docs/work/tracking-ui/r1-findings.md`. Never run `git` write commands.

Read `docs/work/tracking-ui/data-model.md` and the "Phase 2" section of `plan.md`, then review
these commits (`git show <sha>`): 62c28b4 (migration chain + 003), 22ec4c3 (stop override/done,
bump, per-task metrics, session notes, stats item filter), 029912a (widget pause panel,
distraction counter, notes).

Look for correctness bugs only: data loss, wrong op_id replay, a write that bypasses `changes`/
`version`, metrics_custom/item_metrics cleanup rules, bump below 0 or on a stopped session,
notes on the wrong session, Save & Done with a Worked override, timezone/day-boundary mistakes in
note timestamps, widget calls that send the wrong params or mis-handle errors, bar "+" clicks
toggling the popup. Append each finding as soon as you confirm it (you may be cut off by a usage
limit): `- [high|medium|low] file:line — defect — concrete failing scenario — suggested fix`.
Write `DONE` as the last line when finished. No praise, no style nits.
