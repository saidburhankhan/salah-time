# R3 brief (Codex): read-only review of today's app and widget work

You are reviewer R3 for `tracking-ui` in /home/saidburxon/rust/salah (branch `architecture`).
Other workers are editing `crates/**` and `app/**` right now: do NOT edit any file except your
findings file `docs/work/tracking-ui/r3-findings.md`. Never run `git` write commands. Review the
committed frontend changes with `git log --oneline 5dc4668..HEAD -- app plugin` and `git show <sha>`
(or `git show HEAD:<path>`), not the working tree.

Read plan.md ("User decisions", "Phase 2" and its addenda, "Phase 3" incl. "Epics"),
`docs/work/architecture/protocol-v1.md` and the fixtures in `crates/salah-cli/tests/fixtures/rpc/`.

Look for correctness bugs only:
- requests whose params differ from the protocol/fixtures (names, null vs absent, `"none"`, op_id reuse);
- results read with wrong field names;
- stale state after `changed` notifications;
- race conditions between an in-flight request and a newer filter (stale response overwriting newer data);
- timers that drift or flicker (MM:SS, at least two digits);
- the bar "+" click target toggling the popup;
- notes or values sent to the wrong session;
- Worked minutes exceeding `max_minutes`;
- `Text.PlainText` missing on dynamic strings that could contain markup;
- the CLI fallback path sending flags the CLI doesn't have (check `crates/salah-cli` at HEAD);
- anything that would crash or hang the shell.

Append each finding as soon as it is confirmed (you may be cut off):
`- [high|medium|low] file:line — defect — concrete failing scenario — suggested fix`.
Write `DONE` as the last line. No praise, no style nits.
