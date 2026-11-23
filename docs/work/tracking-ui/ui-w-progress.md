# UI-W progress (widget, slice U4)

Owner: UI-W (`plugin/**`). Status: **U4 reworked for the user decisions of 2026-11-23**
(plan.md), ready for the orchestrator to install (U5).

## Rework (user decisions, 2026-11-23)

- **Timer = play/pause.** The row button ("▶" / "❚❚ 12m") and the running card's "❚❚ Pause" send
  `session.stop` at once with no values and no note. No stop panel.
- **Done = explicit tick.** Tapping the done box on an item without a tick opens the done panel:
  the item's metrics (project metrics in position order, or focus and quality without a project)
  and a note, all optional, no Required/Optional headings. "Done" sends `session.log` without
  minutes carrying the filled values; "Skip" ticks with nothing. If the item's timer is running it
  is stopped first (no values), and the tick is sent once the stop succeeds.
- **Untick** removes the item's tick session(s); timed sessions stay. The "Done by timed sessions"
  path is gone: an item with only timed sessions toggles normally.
- **No gating.** Save/Done/Skip are always enabled. "+ Log time" still needs a target and minutes,
  but says so when Save is pressed instead of disabling it; its metrics are optional.
- **Dark surfaces only.** The live "white panels" came from `Commons.Color` used without the
  `qs.Commons as Commons` import (the Rectangle fell back to its default white); fixed in U4 and
  now checked in every plugin file. Panels, the running card and the opened row use the hover
  fill plus a hairline normal border; the current-prayer header uses the hover fill; chosen chips,
  scale buttons, the done box and a marked prayer use the translucent selected fill (no
  `selectedStateColor` blocks, no `Color.background` text on light fills). All colours come from
  `Commons.Color`/`Style` tokens.
- Node tests updated (35 checks): untick vs done, stop-before-tick id, timed sessions never done,
  position order (required ignored), built-ins, targets, values (empty = Skip, checks 0/1).
- Rendered against a scratch daemon with optional metrics (`render/out7`, `out8`): running →
  pause (no panel), start another, tick it while running → done panel → Done (stored: timed
  1 min + tick with focus 4), project item → Skip (tick, no values), untick (tick removed),
  Log panel with a project's metrics, opened row with sessions, current-prayer header. All dark,
  text in theme foreground / muted foreground.
- Transitional: until backend C1 lands, day rows' `done` still means "has any session", so an
  item with only timed sessions shows checked; tapping it opens the done panel (correct under the
  new rules). The scratch/live daemon may still reject missing required metrics for projects that
  have required ones.

## Original U4 (superseded where it conflicts)

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

1. Backend C1 as planned: stop enforcing required metrics and make `done` mean "has a tick".
   The widget already follows those rules; `status.get.prompt` is no longer used.
2. New files since the last install (`MetricField.qml`, `SessionLogic.js`): after install,
   `omarchy restart shell` may be needed.

## Gaps

- Vertical bar layout and hover states not rendered (reviewed in code only).
- Starting a second timer returns the daemon's "a session is already running" inline; no automatic
  "stop the other one first" flow.
- In CLI-fallback mode (no daemon socket) projects/metrics lists are not loaded, so "+ Log" offers
  today's items only.
