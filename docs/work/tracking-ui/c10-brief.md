# C10 brief (Codex): widget goal chips and weekly review link

You are worker C10 for work item `tracking-ui` phase 4 in /home/saidburxon/rust/salah (branch
`architecture`). You own `plugin/**` and `docs/work/tracking-ui/c10-progress.md`.

Read: `docs/AGENTS.md`, `plan.md` ("User decisions", "Phase 4"), `ui-w-progress.md` and
`c3-progress.md` (how the widget is built and rendered), `protocol-v1.md`, and the fixture
`crates/salah-cli/tests/fixtures/rpc/status_goals.json`.

Tasks:
1. Panel header goal chips from `status.get.goals`: today's and this week's goals as compact
   progress chips (label, value/target, a thin progress bar, a check when met), at most 3 then
   "+N"; nothing shown when there are no goals.
2. Monday after Fajr, while last week's `review.get` has no note, a quiet "Weekly review" link in
   the panel footer that opens the app's Review page (`salah open` with the page argument the app
   accepts; ask in your progress file if none exists). No notification.
Add node checks for the chip logic in `plugin/local.salah/tests/`. CHECKPOINT after both.

The user's rules: no bright white; muted theme tints; `Text.PlainText`; the plugin entry point
stays `BarWidget.qml`; never break the bar "+" click target.

Checks: every `plugin/local.salah/tests/*_test.js` with node, `omarchy plugin validate
plugin/local.salah`, `git diff --check`.

## Checkpoint protocol

Your sandbox cannot write `.git` or create Unix sockets. Never run `git commit`. At each green
checkpoint (all checks pass except tests that need Unix sockets) append to your progress file a
line `CHECKPOINT <slice>: <one-line commit message>` listing the files, then END your turn. The
orchestrator (Claude session `salah-orch`) runs the socket tests, reviews, commits, and resumes you.
Do not edit files outside your ownership. Never touch ~/.local/share/salah or the live daemon;
do not run ./scripts/install.
