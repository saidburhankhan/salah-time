# UI-W progress (widget, slice U4)

Owner: UI-W (`plugin/**`). Status: **U4 done**, ready for the orchestrator to install (U5).

## Delivered

- **Tick/untick** (`SessionLogic.doneAction`): no sessions today → `session.log` tick; only ticks →
  `session.remove` each; any timed/running session → no toggle, the row expands to its session list
  ("Done by timed sessions. Remove one to undo."). A second tick is never logged.
- **Running session**: bar shows the project colour dot (painted into a gap in the label) and the
  elapsed time; tooltip names it. Popup has a running card (dot, title, elapsed, **■ Stop**) and the
  running row's timer button reads "■ 12m". Stop opens the metric panel and works whether or not the
  item is done. Elapsed time is derived from `started_at` each second (no snapshot drift).
- **Stop/Log panel** (`SessionPanel.qml`, `MetricField.qml`): required metrics first, then optional
  (captions shown when both exist); scale = 1..scale_max tap buttons (tap again clears), count = field
  with its unit, check = switch (a required check counts as "No" until switched on); optional note.
  Save is disabled with a reason ("Fill in: Focus, Quality", "Enter minutes…", "Pick an item or a
  project."). Daemon errors appear inside the panel; `data.missing` highlights the metrics by name.
  The panel stays open until the daemon confirms.
- **+ Log time**: pick a today item or a project (colour dots), labelled Minutes field with 15/30/60
  chips, metrics for the chosen target (project metrics; built-in focus/quality optional for an
  item without a project). With nothing to pick it says so and offers **Open Salah app**
  (`Quickshell.execDetached(["salah-app"])`, no shell).
- **Session time adjust** (expanded row): each session shows start time / "logged" / "tick", an
  editable minutes field (Enter → `session.edit minutes`) and × (`session.remove`); "+ Time" opens
  the Log panel preset to that item. Row errors show inside the row.
- **Sync**: status refetched on every `changed` (except our own op, which refetches after its
  response) and after each mutation; projects/metrics lists reload on `projects`/`tags`/`metrics`
  scopes and when the Log panel opens. Stop panel closes if the session was stopped elsewhere.
- CLI fallback passes `--value`, `--note`, `--block`, `--id`, `--project`; adds session remove/edit.
- Fixed pre-existing bug: `PrayerRow.qml`/`TimelineRow.qml` used `Commons.Color` without
  `import qs.Commons as Commons` (ReferenceErrors in the live popup). The current-prayer header now
  uses the hover fill (the selected fill renders as a bright bar once it actually resolves).
  `Commons.Color.error` does not exist in the shell; errors use `urgent`.

## Checks

- `node plugin/local.salah/tests/session_logic_test.js` → 40 checks pass (fixture-based: untick
  decision, metric ordering, built-ins, targets, values, parsing).
- `node plugin/local.salah/tests/drag_logic_test.js` → 29 pass.
- `omarchy plugin validate plugin/local.salah` → exit 0. `git diff --check` clean.
- Offscreen render: real `BarWidget.qml` in quickshell (`QT_QPA_PLATFORM=offscreen`, stub
  KeyboardPanel, fake bar) against a scratch `salah serve`; screenshots in the session scratchpad
  `ui-w/render/out{2,3,4,5}`. Verified: bar dot + elapsed, running card, stop panel, daemon
  validation error inline, stop success, tick → untick → tick, timed item expands, log to project
  with required check, empty state with Open app, external `session.start` appears via `changed`,
  edit minutes 25→40, remove session un-dones the item.

## Requests for the orchestrator

1. `status.get.prompt` is `[]` for a running session without a project. The plan says the built-in
   focus/quality are still asked (optional); the widget does this client-side. Consider returning
   them from the backend so app and widget cannot diverge.
2. `scripts/test` does not run the plugin node tests; please add
   `node plugin/local.salah/tests/drag_logic_test.js` and `.../session_logic_test.js`.
3. New files (`MetricField.qml`, `SessionLogic.js`): after install, `omarchy restart shell` may be
   needed.

## Gaps

- Vertical bar layout and hover states not rendered (reviewed in code only).
- Starting a second timer returns the daemon's "a session is already running" inline; no automatic
  "stop the other one first" flow.
- In CLI-fallback mode (no daemon socket) projects/metrics lists are not loaded, so "+ Log" offers
  today's items only.
