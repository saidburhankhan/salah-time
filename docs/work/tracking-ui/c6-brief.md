# C6 brief (Codex): widget creation sheet, reminders, resources

You are worker C6 for work item `tracking-ui` phase 3 in /home/saidburxon/rust/salah (branch
`architecture`). You own `plugin/**` and `docs/work/tracking-ui/c6-progress.md`. Other workers may edit
other directories concurrently; never touch them.

Read: `docs/AGENTS.md`, `docs/work/tracking-ui/plan.md` (the "User decisions", "Phase 2" and
"Phase 3" sections; phase 3's contract is locked, implement it exactly), the previous progress files
in `docs/work/tracking-ui/` for how this area was built and verified, and the code you own.

Tasks: (1) the creation sheet exactly as plan.md describes (tag filter chips, project colour chips,
minutes chips, When = block chips with Today/Tomorrow or Backlog, Add / To backlog, prefill from the
parsed text, Enter keeps the fast path, Esc closes); (2) the bar bell with the due-reminder count and
the due list at the top of the popup with tick-off (`resource.edit done`); (3) the 📎 count on rows
and the resource list in the opened row (links open with Qt.openUrlExternally for http/https only,
reminders tickable, quick add of a note or link). Keep the dark theme tokens (no light fills),
`Text.PlainText` on dynamic strings, the popup compact. Add node checks for new pure logic (prefill
from parse, URL validation, due ordering). Checks: node checks in plugin/local.salah/tests,
`omarchy plugin validate plugin/local.salah`, `git diff --check`. One CHECKPOINT per task; the
orchestrator renders and reviews the screens.

## Checkpoint protocol

Your sandbox cannot write `.git` or create Unix sockets. Never run `git commit`. At each green
checkpoint (all checks pass except tests that need Unix sockets) append to your progress file a
line `CHECKPOINT <slice>: <one-line commit message>` listing the files, then END your turn. The
orchestrator (Claude session `salah-orch`) runs the socket tests, reviews, commits, and resumes you.
Do not edit files outside your ownership. Never touch ~/.local/share/salah or the live daemon;
do not run ./scripts/install.
