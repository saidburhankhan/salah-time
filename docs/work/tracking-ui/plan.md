# Plan: tracking-ui

Storage for every phase 2/3 change is locked in `data-model.md`; read it before any backend work.

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
  carries no values. Nothing in the pause panel is ever required: Save and Save & Done are always
  enabled, there are no required markers or "Fill in" hints, and an empty panel is a plain pause.
  If the session already has values from pause, the done panel still opens on
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
- **Distractions** (user request): a third built-in metric `distractions` (kind count, rollup sum,
  unit "times", builtin, asked by every project by default and for sessions without a project). It
  is counted live while a timer runs, without pausing: `session.bump {id?, metric_id, delta: 1|-1,
  op_id}` adds `delta` to that count metric on the running session (default: the running one;
  never below 0; count metrics only; replay-safe). Because it is an ordinary session value it
  appears in stats series, the general table (also as a derived "per hour" column:
  sum ÷ hours of the sessions in the row), project/task breakdowns and the Notes page range filter,
  and it is editable in every session editor. Existing projects gain it as an asked metric through
  the migration. Fixture `session_bump.json`. It is separate from focus (a 1–5 rating).
- **Bar "+":** a small "+" right after the timer's seconds in the bar. Clicking it sends
  `session.bump +1` and must NOT open or close the popup (it is its own click target, outside the
  label's click area; a brief pulse shows it counted). The popup's running card and the app's
  running card show "Distractions N" with − and + buttons. The pause panel shows the current count
  (editable) with the other metrics.
- **Per-task metrics** (user request): a task (one-time or routine) can choose which of its
  project's metrics it asks. Item gains `metric_ids: [metric_id] | null`; `null` (default) = all of
  the project's asked metrics; a list = only those (each must be an active metric the project asks,
  or a built-in; an empty list = ask nothing). `item.add`/`item.edit` accept `metric_ids?` (`null`
  resets to the project's). Day rows and backlog rows gain `metrics: [PromptMetric]`, the
  effective list in project order, so the frontends never compute it; `status.get.prompt` for a
  running item uses the same list. Panels (pause, done, log for an item) ask exactly that list.
  Changing the project's metrics later changes tasks that inherit (`null`) and drops removed ids
  from tasks with a list. The bar "+" (distractions) works whether or not the task asks it.
  App item editor: under the project chips, "Asks" with the project's metrics as toggle chips (all
  on by default; "Reset to project"). Widget: no editor needed (the app owns it); the creation
  sheet (phase 3) may add it later. Fixtures `item_edit_metrics.json`.
- **Notes during a running session** (user request; storage per data-model.md: separate
  `session_notes` entries shown as `HH:MM text`): `session.note {id?, text, op_id}` adds an entry without stopping it or touching earlier
  lines (default: the running session; text 1–500 chars, trimmed; replay-safe). Fixture
  `session_note.json`. Earlier notes are never replaced by this method; `session.edit note` still
  edits the whole note.
- **Bar "+" mouse buttons:** left click = distractions +1 (as above). **Right click = distractions
  +1 and opens a small note box** for the running session (a compact popover anchored to the bar,
  not the full popup; one line field, Enter or Save sends `session.note`, Esc closes without saving;
  the count stays even if the note is cancelled). Neither click toggles the main popup.
- **"Add note" on the running card** (widget popup and app): the same one-line field without
  counting a distraction.
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

### Epics (user request 2026-11-23 ~08:30; replaces the "ongoing task" idea)
- **Hierarchy:** Project → Epic → tasks/sessions. An epic belongs to exactly one project, has no
  fixed duration or end date, and is not a task: each day's work is its own task or session; nothing
  migrates between days. Colour and tags come from the project.
- Epic `{id, project_id, name, note|null, status: "active"|"finished", finished_at|null, position,
  sync fields}`. Methods: `epics.list {project_id?, status?}` → epics with
  `{total_minutes, sessions, days_worked, last_worked|null, open_tasks, done_tasks}`;
  `epic.add {project_id, name, note?}`, `epic.edit {id, name?, note?, status?, expected_version?}`
  (finishing sets `finished_at`; reopening clears it), `epic.remove {id}` (only with no sessions;
  otherwise finish it).
- Items gain `epic_id?` (must belong to the item's project; cleared if the project changes).
  Sessions gain `epic_id`, copied from the item when written (like `project_id`), or given
  directly by `session.log`/`session.start` with `epic_id` for work logged against the epic
  without a task. Changing an item's epic later never rewrites past sessions.
- Resources (notes, links, reminders) can belong to an epic as well as an item: `resource.add`
  takes `item_id` or `epic_id`.
- **Metrics/stats:** `stats.get`, `stats.table`, `sessions.list` gain `epic_id?` filters;
  `stats.table` gains `group: "epic"`; `stats.get` gains `epics: [{id, name, project_id, minutes,
  sessions, metrics}]`. So every view can go Project → Epic → Task/Routine.
- **App:** Settings → Projects: each project lists its epics (add, rename, finish/reopen). An Epic
  page (from the project, Backlog or a task's chip) shows totals, days worked, a minutes-per-day
  chart, metric trends, its tasks history and its notes/resources. Item editor and Log: an optional
  epic chip row under the project chips. Stats/Notes filter bar: Tags / Projects / **Epics** /
  Tasks. Backlog grouped by project → epic.
- **Widget:** the creation sheet and "+ Log" show epic chips after a project is chosen (optional);
  rows show the epic name next to the project dot.
- **Optional everywhere (user):** an epic is never required. A task may have a project and an
  epic, a project only, or neither; a session may have any of these. Nothing about existing
  projects, tasks, routines or sessions changes: `epic_id` is a new nullable column, every existing
  row stays `null`, and every existing method, result and fixture keeps its shape (additive fields
  only). Routines can have an epic too, but never need one.
- **Statistics show the gaps explicitly:** breakdowns and the general table include a
  **"No project"** row (sessions without a project) and, within a project, a **"No epic"** row
  (that project's sessions without an epic), so totals always add up to the whole. Filters offer
  `epic_id: "none"` (the project's work outside any epic) and `project_id: "none"` (work with no
  project). A filtered view never silently drops unassigned work.
- **Architecture:** follow the existing layout (domain type in salah-domain, table and queries in
  salah-store, logic in salah-core modules next to projects/sessions/stats, methods in the
  protocol's method table, CLI subcommand like `project`). No changes to unrelated modules; no
  file over ~800 lines; migrations additive and in place on the user's live database.
- Fixtures: `epics_list.json`, `epic_add.json`, `epic_edit_finish.json`, `session_log_epic.json`,
  `stats_table_epic.json`.

### Workers (Codex gpt-6.2-astra high, or Claude if Codex is limited)
- C5 backend (`crates/**`): the contract above, schema additive (new table, nullable day/block with a
  CHECK tying them to repeat), tests, CLI (`salah backlog`, `salah resource`), fixtures, protocol-v1.md.
- C6 widget (`plugin/**`): creation sheet, bell + due reminders, row 📎 and resource list.
- C7 app (`app/**`): task drawer, Backlog page, schedule dialog, drag from backlog, "→ Backlog".

### Slices
- [ ] R1 backend (C5): backlog + resources, then epics
- [ ] R2 widget (C6)
- [ ] R3 app (C7)
- [ ] R4 integration, screenshots reviewed, install, user check

## Phase 4: insights, goals, weekly review (user request 2026-11-23 12:2x)

The user chose these three from a list so Codex keeps a queue through the 25 Nov renewal. Same
rules as before: everything optional, dark muted colours, `Text.PlainText`, sessions are the facts
(progress and insights are computed from sessions, never stored), additive protocol, storage per
data-model.md "005". Builds on phase 3 (epics); starts after phase 3's backend is committed.

### Insights (read-only)
- **App Insights page** (sidebar entry), with the shared Stats filter bar (range, tag, project, epic, task):
  - "By prayer block": minutes, average focus and distractions per hour for each block (night…isha, sleep);
  - "By hour of day" (timed sessions only) and "By weekday": the same three measures;
  - "Distractions per hour" by week, as a line;
  - "Best sessions" and "Hardest sessions": the top 5 by focus (ties broken by minutes), each with its notes, opening the session.
- Small samples are shown as such ("3 sessions"), never hidden; no fake precision (one decimal).
- `insights.get {from, to, tag_id?, project_id?|"none", epic_id?|"none", item_id?}` →
  `{by_block:[{block, minutes, sessions, focus_avg|null, distractions_per_hour|null}],
    by_hour:[{hour 0..23, …same}], by_weekday:[{weekday 1..7 (Mon=1), …same}],
    distractions_by_week:[{week (Monday), per_hour|null}],
    best:[SessionView+item/project names], hardest:[…]}`. Ranges up to 730 days. Tick sessions
  (no minutes) are excluded; `focus_avg` is minutes-weighted like stats.

### Goals
- A goal is a target for a period (day or week, weeks start Monday) on all work, one project, one
  epic or one task/routine:
  - minutes, at least N;
  - or a metric with a direction, e.g. focus average at least 4, distractions at most 10 per week.
- Progress is computed from sessions. Changing a target ends the old goal and starts a new one, so
  past periods keep the target they had.
- **App:**
  - a Goals section in each project/epic/task editor and an overall Goals page;
  - progress bars in muted tints, with goals met shown by a check;
  - history per goal (period, value, target, met) as a small table.
- **Widget:** the panel header shows today's and this week's goals as compact progress chips (max 3, then "+N").
- **Protocol:**
  - `goals.list {at?: day}` → `[{id, project_id, epic_id, item_id, period, metric_id|null,
    direction, target, starts_on, ends_on|null, progress:{from, to, value, met}}]`. Active goals
    only unless `include_ended`.
  - `goal.add {scope ids?, period, metric_id?, direction?, target, op_id}`.
  - `goal.edit {id, target, op_id}`: ends this goal at the end of the previous period, adds the
    successor, and returns it.
  - `goal.end {id, op_id}`.
  - `goal.remove {id, op_id}`: only while its first period has not ended (soft delete).
  - `goals.history {id, from, to}` → `[{from, to, value, target, met}]`.
  - `status.get` gains `goals` (the same rows as `goals.list` for today; the widget's source).
  - `changed` entity `goals`.

### Weekly review
- **App Review page:**
  - a week picker (default: the last full week; the current week is allowed);
  - a computed summary: minutes by project and epic with "No project"/"No epic", goals met, the
    sessions that have notes (newest first, filterable), open backlog count, distractions per hour
    vs the previous week;
  - one free-text reflection the user writes and saves (optional).
- **Widget:** on Monday after Fajr, a quiet "Weekly review" link in the panel footer while last
  week has no saved reflection. No notification.
- **Protocol:**
  - `review.get {week}` → `{week, summary:{…as above}, note|null, updated_at|null}`.
  - `review.save {week, note, op_id}`: an empty note removes it.
  - `changed` entity `reviews`.

### Workers
- **C8 backend (`crates/**`, Astra for 005 and insights SQL, Sol for CLI/fixtures):**
  - migration 005;
  - goals, insights and review in salah-core modules `goals`, `insights`, `reviews`;
  - CLI `salah goal …`, `salah insights`, `salah review`;
  - fixtures `insights.json`, `goals_list.json`, `goal_add.json`, `goal_edit_successor.json`,
    `goals_history.json`, `review_get.json`, `review_save.json`, `status_goals.json`;
  - protocol-v1.md.
- **C9 app (`app/**`, Astra for the Insights page, Sol for Goals and Review):** screenshots of each page reviewed.
- **C10 widget (`plugin/**`, Sol):** goal chips and the review link; offscreen render reviewed.

### Slices
- [ ] G1 backend (C8): 005 + goals (CHECKPOINT 0 alone for the independent review), then insights, then review
- [ ] G2 app (C9)
- [ ] G3 widget (C10)
- [ ] G4 integration, screenshots reviewed, install, user check
