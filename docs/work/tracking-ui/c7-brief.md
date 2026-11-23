# C7 brief (Codex): app task drawer, backlog page, scheduling

You are worker C7 for work item `tracking-ui` phase 3 in /home/saidburxon/rust/salah (branch
`architecture`). You own `app/**` and `docs/work/tracking-ui/c7-progress.md`. Other workers may edit
other directories concurrently; never touch them.

Read: `docs/AGENTS.md`, `docs/work/tracking-ui/plan.md` (the "User decisions", "Phase 2" and
"Phase 3" sections; phase 3's contract is locked, implement it exactly), the previous progress files
in `docs/work/tracking-ui/` for how this area was built and verified, and the code you own.

Tasks: (1) task drawer beside a selected task on Today (and Backlog) listing its resources with
add/edit/remove for note, link (opens the browser), reminder (date-time picker, done toggle);
(2) Backlog page in the sidebar: grouped by project, tag/project filter, manual order (drag),
planned minutes and resource counts, Schedule (Today / Tomorrow / date + block chips), drag a
backlog task onto a block on Today, "→ Backlog" on one-time Today items; (3) tests for every new
request shape and screenshot coverage (drawer, backlog page, schedule dialog) so the orchestrator can
render them with `SALAH_UI_SHOT_DIR=<dir> app/bin/test`. Readable QML, muted theme tints (never
white fills), `Text.PlainText` for dynamic strings, existing tests kept. Checks: `app/bin/build`,
`app/bin/test` (socket tests skipped in your sandbox), `git diff --check`. One CHECKPOINT per task.

## Checkpoint protocol

Your sandbox cannot write `.git` or create Unix sockets. Never run `git commit`. At each green
checkpoint (all checks pass except tests that need Unix sockets) append to your progress file a
line `CHECKPOINT <slice>: <one-line commit message>` listing the files, then END your turn. The
orchestrator (Claude session `salah-orch`) runs the socket tests, reviews, commits, and resumes you.
Do not edit files outside your ownership. Never touch ~/.local/share/salah or the live daemon;
do not run ./scripts/install.
