# W2 brief (frontends, S5)

You are worker W2 for work item `tracking-model` in /home/saidburxon/rust/salah
(branch `architecture`). The orchestrator is the Claude session `salah-orch`; the
backend worker W1 (`salah-lead`) is concurrently rewriting `crates/**`.

Read, in order: `docs/AGENTS.md`, `docs/work/tracking-model/plan.md`,
`docs/work/tracking-model/interface.md` (the locked contract, including the
"UX decisions" section). Do not change `interface.md`; list requested
changes in your report.

## Outcome

The app and the widget are built against the new protocol in `interface.md`:

1. **Widget** (`plugin/local.salah/`): shows the running session (project
   colour dot and elapsed time); tapping an item starts/stops its session; on stop, an
   inline panel asks the `prompt` metrics (scale = 1–5 tap buttons, count = number
   field, check = toggle, optional note) and sends `session.stop`; "+ Log"
   sends `session.log` (project, minutes, metrics). Ticking an item = `session.log`
   with no minutes. Remove `entry.*`/`timer.*` usage.
2. **App Today** (`app/src/qml/today/`, models): each block has a capacity bar
   (capacity vs planned vs actual), plan lines show project colour, planned minutes
   and sessions, and a day fulfilment summary sits at the top. Item editor: project
   picker and planned minutes instead of measure/target.
3. **App Settings**: Projects (name, colour, tags, metrics asked + required),
   Tags, Metrics (add/edit/archive; built-ins can't be archived).
4. **App Stats**: a filter (All / tag / project), a range (7/28/90), KPI tiles
   (minutes, active days, avg focus, fulfilment), a line chart per `series`, minutes
   per day vs capacity, and project and block breakdowns.

## Ownership

- You own: `app/**`, `plugin/**`.
- Do not edit: `crates/**`, `scripts/**`, `docs/**` (except
  `docs/work/tracking-model/w2-progress.md`, which you keep updated with your last
  commit, current step and next step).

## Working against an unfinished backend

The backend lands in stages. Until W1 commits the regenerated fixtures under
`crates/salah-cli/tests/fixtures/rpc/`, build against `interface.md` and
write your own fixture JSON under `app/tests/fixtures/` matching it exactly.
When the real fixtures appear, switch the tests to them and fix any mismatch.
Report contract mismatches; do not work around them silently.

## Rules

- Follow `docs/AGENTS.md` safety rules (`Text.PlainText` for dynamic strings, entry
  point named exactly `BarWidget.qml`, no prayer maths in QML).
- Keep the UI compact and fast to record into; no extra measurement tools.
- Commit at green checkpoints: `app/bin/build`, `app/bin/test`,
  `omarchy plugin validate plugin/local.salah` (if available), plus the plugin's
  node checks in `plugin/local.salah/tests`. Never commit a failing state.
- Do not run `./scripts/install` or restart the shell; the orchestrator does live
  installs.

## Final report (in w2-progress.md and your last message)

Commits, verification output, contract mismatches, remaining gaps.

## Checkpoint protocol (overrides any "commit" instruction above)

Your sandbox cannot write `.git` or create Unix sockets. So: never run `git commit`.
At each green checkpoint (all checks pass except tests that need Unix sockets),
append to your progress file a line `CHECKPOINT <slice>: <one-line commit message>`
listing the files in that checkpoint, then END your turn (exit). The orchestrator runs the
socket tests, reviews, commits, and resumes you with the next step. Do not start the next slice
before you are resumed.
