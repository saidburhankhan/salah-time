# Plan: tracking-ui

## Outcome

The app and the widget make the tracking model (archived in `docs/archive/tracking-model/`,
contract in its `interface.md`) usable day to day: create projects/tags/metrics easily, plan
items with a project, start/stop timers, add or remove time, tick and untick, log sessions with
the required and optional metrics, and see results in charts. Both surfaces stay in sync.

## What the user hit (2026-11-23 morning, live install bb419b4)

1. App item editor: project picker is a cramped ComboBox showing "Routine / …"; there are no
   projects and no obvious way to make one.
2. Widget "+ Log": Save is disabled (needs a project; none exist), no minutes label, no metrics.
3. Ticking an item logs a new tick session on every tap; there is no untick (one item had 11).
4. A running timer could not be stopped from the app or the widget.
5. No visible projects/tags pages, no way to add or remove time, no optional metrics.
6. Regressions by W2: Settings lost the Sleep time / location / method section; Stats lost its
   charts (orphaned components in `app/src/qml/statistics/`); Settings and Stats were written as
   minified one-line QML.

## Decisions (orchestrator)

- **Done/untick:** tapping done on an item with no sessions today sends `session.log` (tick).
  Tapping it again when the item's only sessions today are ticks removes those ticks
  (`session.remove` each). An item with timed sessions is done; to undo, remove sessions in its
  session list. Never log a second tick while one exists.
- **Project is optional for logging:** "+ Log" lets you pick a today item *or* a project. With
  nothing to pick, it says so and offers to open the app's Projects page (`salah-app` launch is
  fine). Logging against an item copies its project (backend does this).
- **Metrics:** prompts show required metrics first (must be filled) and then the project's
  optional metrics (may be left empty). Scale = 1..scale_max tap buttons, count = number field
  with a unit label, check = toggle. An item/session without a project still asks the built-in
  focus and quality as optional.
- **Time adjust:** every session in an item's session list shows minutes and can be edited
  (`session.edit minutes`) or removed. "+ time" on an item logs a session with minutes
  (quick 15/30/60 buttons and a field).
- **Running timer:** visible in the app (Now card or top of Today) and the widget bar, with a
  Stop button that opens the metric prompt; Stop also works when the item is done. Both surfaces
  refresh from `status.get.running` on every `changed` notification.
- **Settings:** restore the prayer/sleep section from `c673a99` and add three proper pages:
  Projects (list; editor with name, colour swatches, note, tag chips, metric rows each with
  Ask/Required toggles, archive), Tags (name, colour, rename, remove), Metrics (name, unit, kind,
  scale max, rollup; archive; built-ins locked).
- **Stats:** rebuild with the existing chart components: KPI tiles, a line chart per metric
  series, minutes/day vs capacity bars, project and tag breakdowns with colour, block breakdown,
  filter All/tag/project actually applied (`stats.get tag_id/project_id`), range 7/28/90.
- **Code quality:** normal readable QML (one property per line in new code), no minified files.
  `Text.PlainText` for dynamic strings. No prayer maths in QML.

## User decisions, 2026-11-23 (supersede the bullets above where they conflict)

- **Timer = play/pause.** Stopping (pausing) a timer asks nothing: no metrics, no note. It sends
  `session.stop` with no values. An item can have many timed sessions a day.
- **Done = an explicit tick.** Tapping done opens the done panel: the item's metrics (its
  project's metrics, or focus and quality when it has no project) and an optional note. **Every
  field is optional**; the panel has "Done" (saves what is filled) and "Skip" (ticks with no
  values). It sends `session.log` without minutes (a tick) carrying the values. If the item's
  timer is running, it is stopped first (no values), then ticked.
- **Untick** removes the tick session (and its values). Timed sessions stay; they never make an
  item done by themselves.
- **No required metrics.** The backend no longer rejects `session.stop`/`session.log` for missing
  metrics. The project's metric list only decides what the done panel asks. The `required` field
  stays in the protocol for compatibility but is not enforced and the UI does not show a
  Required toggle (Settings shows "asked" metrics only).
- "+ Log time" still logs minutes to an item or project; its metrics are optional too.
- Backend `done` (day rows, `fulfilment.done_items`) = the item has a tick session that day.

## Workers

- **UI-A (Claude):** `app/**`. Settings, item editor, Today (running timer, sessions, time
  adjust, tick/untick), Stats.
- **UI-W (Claude):** `plugin/**`. Bar running indicator, tick/untick, start/stop, Stop/Log
  panel with item-or-project picker and required+optional metrics, session time adjust if it fits
  the popup.
- The backend (`crates/**`) is owned by the orchestrator; ask in your report if something is
  missing — do not work around it.

## Verification each worker must do

- App: `app/bin/build`, `app/bin/test` (all pass, count of test functions not reduced), and
  **screenshots** of Today (with a running session, a ticked item, an item with sessions), item
  editor, Settings (each page), Stats with data, rendered offscreen (see the `shot` helper in
  `app/tests/salah_app_tests.cpp`) into the scratchpad; Read every PNG and fix what looks wrong.
- Widget: node checks in `plugin/local.salah/tests`, `omarchy plugin validate plugin/local.salah`,
  and an offscreen render of the popup / panels if feasible (otherwise a careful QML review).
- Live data: never touch `~/.local/share/salah`. For a real daemon use a scratch environment:
  `XDG_DATA_HOME XDG_STATE_HOME XDG_CONFIG_HOME XDG_RUNTIME_DIR` all pointing into the scratchpad,
  then `salah serve` there.
- Commit only your own paths, explicitly (`git add app/...` / `git add plugin/...`), never `-a`.
  Do not run `./scripts/install`; the orchestrator installs.

## Slices

- [x] U1 — app: Settings (restore prayer section, Projects/Tags/Metrics editors), item editor picker.
- [x] U2 — app: Today running timer + stop prompt, session list edit/remove, + time, tick/untick.
- [x] U3 — app: Stats rebuilt with charts and working filter.
- [x] U4 — widget: tick/untick, running indicator + stop, Log panel (item or project, metrics).
- [ ] U5 — integration: install, live smoke in scratch env, user visual check.
