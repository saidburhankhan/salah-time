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

## Phase 2 (user request 2026-11-23 07:2x; starts 11:27 with Codex)

### Decisions
- **Pause asks optional metrics.** Pause opens a compact panel: the item's metrics (or focus and
  quality without a project) and a note, all optional. Buttons: **Save** (pause; empty = plain
  pause) and **Save & Done** (pause + tick). Values and note go on the timed session; the tick
  carries no values. If the session already has values from pause, the done panel still opens on
  a later tick but may be skipped.
- **Stats filters:** separate rows/lists for Tags, Projects (narrowed by the chosen tag), Tasks
  (narrowed by the chosen project). Every chart and table follows the filter.
- **Range:** presets 7 / 28 / 90 plus **Custom** with from/to date pickers (any range up to 730
  days).
- **Chart/Table toggle** on each chart: the table shows the same per-day numbers.
- **General table** (new section on Stats): group by Week (default), Day, Project, Tag, Task;
  column chips for Minutes, Sessions, Focus, Quality and each custom metric (remembered);
  scale metrics tinted muted red→amber→green by value/scale_max; minutes with an in-cell bar
  scaled to the column max; totals row; click a header to sort.

### Contract additions (additive; the orchestrator locks these)
- `stats.get` params gain `item_id?`; `from`/`to` accept any range up to 730 days.
- `stats.table` params: `from,to` or `days`, `tag_id?`, `project_id?`, `item_id?`,
  `group: "week"|"day"|"project"|"tag"|"item"` (default `"week"`; weeks start Monday).
  Result:
  ```
  { range, filter, group,
    columns: [{ id, name, unit, kind, rollup, scale_max|null }],   // "minutes", "sessions", then active metrics in position order
    rows:    [{ key, label, color|null, from|null, to|null, minutes, sessions, values: {metric_id: number|null} }],
    totals:  { minutes, sessions, values: {metric_id: number|null} } }
  ```
  Only finished sessions count. `avg` metrics are weighted by session minutes (ticks weigh 1),
  `sum` metrics add up. With `group:"tag"` a session counts in each of its project's tags; totals
  never double count. Rows with no sessions: included for week/day (zeros, null values), omitted
  for project/tag/item. Computed in SQL/core, not in the frontends.
- `session.stop` params gain `done?: bool`: stop the timer with the given values/note, then
  record a tick (no values) for its item in the same write. `done` on a session without an item
  is a `validation` error. The mutation result lists both sessions.
- `session.stop` params also gain `minutes?: int` (1..the clamped elapsed minutes): overrides the
  computed minutes, for a timer the user forgot to pause. Above the elapsed time → `validation`.
  Fixture `session_stop_minutes.json`.
- **Forgot to pause:** the pause panel shows "Worked" with the elapsed minutes in an editable field
  and −5 / −15 / −30 chips; Save sends `minutes` only when it was changed. Already-stopped
  sessions keep their Edit (minutes) in both surfaces.
- **Notes:** a session's note is shown wherever its sessions are listed (app session rows already
  do; the widget's opened row must too).
- **Notes page** (user request): a new "Notes" page in the app sidebar listing sessions with
  their notes, newest first. It shares the Stats filter bar (Tags / Projects / Tasks, range presets
  and Custom) and adds range filters: duration (min–max minutes) and any metric (min–max, e.g.
  focus 1–2), a text search, and a "with notes only" switch (on by default). Each row shows date,
  time, project colour and name, task, minutes, metric values (scale values tinted like the
  table) and the note; clicking a row opens the session editor.
- `sessions.list` params: `from,to` or `days`, `tag_id?`, `project_id?`, `item_id?`,
  `with_note?` (default true), `q?` (case-insensitive note text search), `minutes_min?`,
  `minutes_max?`, `values?: {metric_id: {min?, max?}}` (a session lacking that value does not
  match), `limit?` (default 100, max 500), `before?` (cursor: the last row's `ended_at|id`).
  Result: `{ rows: [SessionView + { day, project: {id,name,color}|null, item: {id,title}|null }],
  next: cursor|null }`. Only finished sessions (ticks included). Fixture `sessions_list.json`
  and `sessions_list_filtered.json`.
- New fixtures: `stats_table_week.json`, `stats_table_project.json`, `stats_item_filter.json`,
  `session_stop_done.json`.

### Workers (Codex gpt-6.2-astra high, checkpoint protocol)
- C2 backend (`crates/**`, protocol-v1.md) — starts 11:27.
- C3 widget (`plugin/**`): pause panel with Save / Save & Done — starts 11:27, codes against the
  contract above; fixtures arrive from C2.
- C4 app (`app/**`): pause panel, filters, custom range, chart/table toggle, general table —
  dispatched by the orchestrator after C2's first checkpoint.

### Slices
- [ ] P1 backend: stats.table, item filter, custom range, session.stop done (C2)
- [ ] P2 widget pause panel (C3)
- [ ] P3 app pause panel + stats filters/range/table (C4)
- [ ] P3b app Notes page (C4, after the stats work)
- [ ] P4 integration, screenshots reviewed by the orchestrator, install, user check

## Phase 3: resources, backlog, widget task creation (user request 2026-11-23 ~07:50)

Starts after phase 2's backend (C2) is committed, so two workers never edit the item model at once.

### Decisions (user)
- Tags stay on projects only; a task shows its project's tags.
- Only one-time tasks can be in the backlog.
- Reminders show only in the widget (no desktop notifications).

### Design
- **Resources** on any item: `note` (text), `link` (http/https URL + optional title; opened in the
  browser), `reminder` (date-time + text; due/overdue ones show in the widget until ticked off).
  App: a task drawer beside the task with the resource list and add/edit/remove. Widget: a 📎 count
  on rows with resources, the opened row lists them (links clickable, reminders tickable), quick add
  of a note or link.
- **Backlog:** a one-time item with no day and no block. It is not in any day plan, capacity or
  fulfilment. App Backlog page: grouped by project, filter by tag/project, manual order, planned
  minutes and resource count per row; **Schedule** (Today / Tomorrow / date + block chips) and drag
  from the backlog onto Today. Today items (one-time) get "→ Backlog".
- **Widget creation sheet:** typing in the add field unfolds a compact sheet under it: tag chips
  (filter), project colour chips, planned-minutes chips 15/30/60/90, When = block chips with Today /
  Tomorrow, or Backlog; buttons **Add** and **To backlog**; prefilled from the parsed text ("read
  20m after isha" → 20 min, Isha); Enter keeps the old fast path; Esc closes; never covers the
  timeline's current block.
- **Reminder in the bar:** a small bell with a count next to the next prayer when a reminder is
  due; the popup lists due reminders at the top with tick-off.

### Contract additions (locked by the orchestrator; additive)
- Item: `day` and `block` become nullable **only** when `repeat = "none"`; `backlog: bool` in
  ItemView (= both null). `item.add` gains `backlog?: bool` (then no day/block). New methods
  `item.schedule {id, day, block, op_id}` and `item.backlog {id, op_id}` (one-time only; a past day
  that already has the item in its frozen plan keeps it). Backlog items never enter day plans.
- `backlog.list {tag_id?, project_id?}` → `[ItemView + {project: {id,name,color,tag_ids}|null,
  resources: {notes, links, reminders}}]` in position order; `item.move` works within the backlog
  with `block: null`.
- Resource `{id, item_id, kind: "note"|"link"|"reminder", title|null, body|null, url|null,
  remind_at|null, done: bool, position, sync fields}`. Methods: `resources.list {item_id}`,
  `resource.add {item_id, kind, title?, body?, url?, remind_at?}`, `resource.edit {id, title?,
  body?, url?, remind_at?, done?, expected_version?}`, `resource.remove {id}`. Validation: link needs
  an http(s) URL; reminder needs `remind_at`; note needs `body`.
- Day rows gain `resources: {notes, links, reminders}` counts.
- `status.get` gains `reminders: [{id, item_id, item_title, title, remind_at, overdue}]`: not done,
  due today or earlier, oldest first.
- `changed` gains the entity `resources`; item changes cover backlog moves.
- Fixtures: `backlog_list.json`, `item_add_backlog.json`, `item_schedule.json`, `item_backlog.json`,
  `resources_list.json`, `resource_add_link.json`, `resource_add_reminder.json`,
  `resource_edit_done.json`, `status_reminders.json`.

### Workers (Codex gpt-6.2-astra high, or Claude if Codex is limited)
- C5 backend (`crates/**`): the contract above, schema additive (new table, nullable day/block with a
  CHECK tying them to repeat), tests, CLI (`salah backlog`, `salah resource`), fixtures, protocol-v1.md.
- C6 widget (`plugin/**`): creation sheet, bell + due reminders, row 📎 and resource list.
- C7 app (`app/**`): task drawer, Backlog page, schedule dialog, drag from backlog, "→ Backlog".

### Slices
- [ ] R1 backend (C5)
- [ ] R2 widget (C6)
- [ ] R3 app (C7)
- [ ] R4 integration, screenshots reviewed, install, user check
