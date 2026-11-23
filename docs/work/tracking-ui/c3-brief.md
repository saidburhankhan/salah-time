# C3 brief (Codex): widget pause panel with Save / Save & Done

You are worker C3 for work item `tracking-ui` phase 2 in /home/saidburxon/rust/salah (branch
`architecture`). You own `plugin/**` and `docs/work/tracking-ui/c3-progress.md`. Worker C2 edits
`crates/**` concurrently (it adds `session.stop done`); do not touch crates.

Read: `docs/AGENTS.md`, `docs/work/tracking-ui/plan.md` (the "User decisions" and "Phase 2"
sections), `docs/work/tracking-ui/ui-w-progress.md` (how the widget was built and how its
screens were rendered offscreen), and the widget files `plugin/local.salah/{BarWidget,SessionPanel,
MetricField,SalahClient,TimelineRow,RowActions}.qml` and `SessionLogic.js`.

Task: Pause (row button and the running card's Pause) opens the session panel in a new "pause"
mode: the session's item/project metrics (or focus and quality without a project) and a note, all
optional, with Cancel, **Save** (`session.stop` with the filled values/note; empty = plain stop)
and **Save & Done** (`session.stop` with `done: true`; for a session without an item, hide it).
The pause panel also shows "Worked" with the elapsed minutes in an editable field plus −5/−15/−30 chips; send `minutes` only when changed (plan.md). Show each session's note in the opened row's session list. Distractions (plan.md): a small "+" right after the timer in the bar that sends `session.bump +1`
and never toggles the popup (its own click target; brief pulse feedback); the running card shows
"Distractions N" with − and +. Keep the existing done panel and Log panel as they are. Add node checks in
`plugin/local.salah/tests/session_logic_test.js` for any new pure logic (params built for Save
vs Save & Done, empty values). Update the CLI fallback in SalahClient (`salah session stop --done`).
Keep the dark theme tokens (no light fills) and `Text.PlainText` on dynamic strings.

Checks: node checks in plugin/local.salah/tests, `omarchy plugin validate plugin/local.salah`,
`git diff --check`. Then one CHECKPOINT. The orchestrator renders and reviews the screens.

## Checkpoint protocol

Your sandbox cannot write `.git` or create Unix sockets. Never run `git commit`. At each green
checkpoint (all checks pass except tests that need Unix sockets) append to your progress file a
line `CHECKPOINT <slice>: <one-line commit message>` listing the files, then END your turn. The
orchestrator (Claude session `salah-orch`) runs the socket tests, reviews, commits, and resumes you.
Do not edit files outside your ownership. Never touch ~/.local/share/salah or the live daemon;
do not run ./scripts/install.
