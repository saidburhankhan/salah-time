# C4 brief (Codex): app pause panel and statistics table

You are worker C4 for work item `tracking-ui` phase 2 in /home/saidburxon/rust/salah (branch
`architecture`). You own `app/**` and `docs/work/tracking-ui/c4-progress.md`. The backend
(`crates/**`, worker C2) has landed the phase 2 contract with fixtures in
`crates/salah-cli/tests/fixtures/rpc/` (stats_table_*.json, stats_item_filter.json,
session_stop_done.json); if some are still missing, build against plan.md's contract and say so.

Read: `docs/AGENTS.md`, `docs/work/tracking-ui/plan.md` ("User decisions" and "Phase 2"),
`docs/work/tracking-ui/ui-a-progress.md` (how the app UI was built, its theme tokens
selectedFill/selectedBorder/selectedText, and how screenshots are rendered with
`SALAH_UI_SHOT_DIR=<dir> app/bin/test`), then the app sources (`app/src/qml/pages/StatisticsPage.qml`,
`app/src/qml/statistics/`, Today's prompt panel, `app/src/backend.*`, `app/src/client/coreclient.*`).

Tasks, in order, one CHECKPOINT each:
1. Today: Pause opens the optional-metrics panel with Save / Save & Done (as plan.md says);
   Save & Done hidden for a session without an item. The panel shows "Worked" with the elapsed
   minutes editable plus −5/−15/−30 chips; send `minutes` only when changed.
1b. Running card: "Distractions N" with − and + (`session.bump`), and the count shown/editable
   in the pause panel and session editor.
1c. Item editor: "Asks" toggle chips of the project's metrics (all on by default, "Reset to
   project"), sent as `metric_ids`; every panel asks the row's `metrics` list.
1d. Running card "Add note": one-line field sending `session.note` (appends; no pause).
2. Stats filters: separate Tags, Projects (narrowed by tag) and Tasks (narrowed by project)
   lists; range presets 7/28/90 plus Custom with from/to date pickers; every request carries
   the filter.
3. Chart/Table toggle on each chart.
4. General table section: group Week (default) / Day / Project / Tag / Task, column chips
   (remembered via QSettings), muted red→amber→green tint for scale metrics, in-cell minutes bar,
   totals row, sortable headers. Use `stats.table`.
5. Notes page (plan.md): sidebar entry, the shared filter bar, duration and per-metric range
   filters, text search, with-notes switch, rows with metric values and the note, "Load more" via
   the cursor, clicking a row opens the session editor. Use `sessions.list`.
Write normal readable QML (one property per line; no minified files); muted theme tints, never
white fills; `Text.PlainText` for dynamic strings. Add tests for each new request shape. Keep
all existing tests passing and do not reduce their number. Extend the screenshot test so the
orchestrator can render: the pause panel, the stats filters with Custom range, a chart in table
mode, the general table grouped by week and by project, and the Notes page with a focus range filter.

Checks: `app/bin/build`, `app/bin/test` (socket tests are skipped in your sandbox; that is
expected), `git diff --check`.

## Checkpoint protocol

Your sandbox cannot write `.git` or create Unix sockets. Never run `git commit`. At each green
checkpoint (all checks pass except tests that need Unix sockets) append to your progress file a
line `CHECKPOINT <slice>: <one-line commit message>` listing the files, then END your turn. The
orchestrator (Claude session `salah-orch`) runs the socket tests, reviews, commits, and resumes you.
Do not edit files outside your ownership. Never touch ~/.local/share/salah or the live daemon;
do not run ./scripts/install.
