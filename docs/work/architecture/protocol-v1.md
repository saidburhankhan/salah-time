# Protocol v1: JSON-RPC 2.0 over NDJSON

This is the transport-independent contract introduced in S3. The same methods
and DTOs are served over stdin/stdout by `salah rpc` and over a user-only Unix
socket by `salah serve` (S4). The human `salah … --json` contract stays frozen
in [`contract-v1.md`](contract-v1.md); it is not the RPC contract.

## Transports

### stdio

`salah rpc` is the default, dependency-free transport: it reads request lines
on stdin and writes response lines on stdout until EOF. It handles requests
in-process, so it is the fallback when no daemon is running and the harness
used by tests and scripts.

`salah rpc --socket` is a stdio↔socket bridge: it connects to the running
daemon and copies frames in both directions, so scripts can keep speaking
NDJSON on stdin/stdout without knowing about Unix sockets.

### Unix domain socket

`salah serve` listens on:

```text
$XDG_RUNTIME_DIR/salah/salah.sock
```

`XDG_RUNTIME_DIR` is required; there is deliberately no `/tmp` fallback and the
daemon never listens on TCP. The runtime directory is mode `0700` and the
socket is mode `0600`, so only the same user can connect. On startup a live
socket is detected by connecting to it; if another daemon answers, the new
process refuses to start. A stale socket file is removed and replaced. On
SIGTERM/SIGINT the standalone daemon removes its socket.

The daemon is the single writer: every mutation is serialised through one
writer thread, while each connection reads through its own SQLite connection.
The single `salah` CLI never talks to the daemon, so the daemon also polls
`PRAGMA data_version` about every 250 ms and emits `changed` when a direct CLI
write advances the store revision.

### systemd user activation

`scripts/install` writes `~/.config/systemd/user/salah.socket` and
`salah.service` (ownership-checked and digested like other installed files)
and, for the real home only, enables the socket. The units are equivalent to:

```ini
# salah.socket
[Socket]
ListenStream=%t/salah/salah.sock
SocketMode=0600
DirectoryMode=0700

# salah.service
[Service]
ExecStart=%h/.local/bin/salah serve
```

The service accepts systemd's `LISTEN_FDS`/`LISTEN_PID` descriptor 3 as well as
binding the socket itself. A socket-activated instance exits after five idle
minutes without connections (configurable with `--idle-minutes`) so systemd can
restart it on demand; a manually started daemon stays up.

## Framing and handshake

- UTF-8, one compact JSON object per `\n`-terminated line, at most 1 MiB per
  line. Batch requests are not supported.
- A request is `{"jsonrpc":"2.0","id":1,"method":"…","params":{…}}`. The `id`
  is echoed unchanged. `params` may be omitted or `null` for no-argument
  methods; non-empty params must be an object.
- The first successful method on a connection must be `hello`:

```json
→ {"jsonrpc":"2.0","id":1,"method":"hello",
   "params":{"protocol":[1],"client":"widget/0.4","actor":"widget"}}
← {"jsonrpc":"2.0","id":1,
   "result":{"protocol":1,"server":"salah 0.1.0",
             "capabilities":["tracking.v1","events.v1","clock.v1"],"revision":184}}
```

Any other method before `hello` fails with JSON-RPC `-32600`. The server
supports protocol major `1`; if the client does not offer it, `hello` fails with
`-32602`. `capabilities` is additive: clients ignore names they do not know.
`tracking.v1` means the projects, sessions, and metrics surface below.
`events.v1` means the connection supports `subscribe` and `changed` pushes.
`clock.v1` means the daemon can push `clock.boundary` (see Notifications).

## Methods

Mutation parameters all include `op_id` (a stable client UUID, or a platform id
such as `telegram:<update_id>`). `revision` in a mutation result is the
store-wide counter, never a precondition.

| Method | Params | Result | Errors | Idempotent | Version-checked |
|---|---|---|---|---|---|
| `hello` | `protocol[]`, `client`, `actor?` | `protocol`, `server`, `capabilities`, `revision` | protocol | — | — |
| `status.get` | — | `Status`: prayer projection, `planner` (today's rows), `running: SessionView\|null`, `prompt: [PromptMetric]`, `reminders: [DueReminder]` | config, source, storage | — | — |
| `day.get` | `date?` | `DayPlan`: prayers, blocks with `capacity_minutes`/`planned_minutes`/`actual_minutes`/`unplanned_minutes` and rows, `open`, `fulfilment` | validation, source, storage | — | — |
| `items.list` | — | `[ItemView]`, the backlog last | storage | — | — |
| `backlog.list` | `project_id?`, `tag_id?` (each also `"none"`) | `[ItemView + project: {id,name,color,tag_ids}\|null + resources: {notes,links,reminders}]` in backlog order | not_found, storage | — | — |
| `resources.list` | Exactly one of `item_id` or `epic_id` | `[Resource]` in position order | validation, not_found, storage | — | — |
| `resource.add` | `op_id`, exactly one of `item_id` or `epic_id`, `kind`, `title?`, `body?`, `url?`, `remind_at?` | mutation result (`changed.resources`) | validation, not_found, storage | yes | no |
| `resource.edit` | `op_id`, `id`, `title?`, `body?`, `url?`, `remind_at?`, `done?`, `expected_version?` | mutation result (`changed.resources`) | validation, not_found, conflict, storage | yes | yes |
| `resource.remove` | `op_id`, `id`, `expected_version?` | mutation result (`changed.deleted`) | not_found, conflict, storage | yes | yes |
| `stats.get` | `from,to` or `days?` (default 30, max 730), `tag_id?`, `project_id?`, `epic_id?`, `item_id?` (each also `"none"`), `minutes_min?`, `minutes_max?`, `values?: {metric_id: {min?, max?}}` | `range`, `filter`, `days` (with `unplanned_minutes`, nullable `fulfilment_ratio`), `series` (minutes first), `projects`, `epics`, `tags`, `blocks`, `totals` | validation, not_found, storage | — | — |
| `stats.table` | `stats.get`'s range and filter (including the session bounds), `group?` (`week` default, `day`, `block`, `project`, `epic`, `tag`, `item`) | `range`, `filter`, `group`, `columns`, `rows`, `totals` | validation, not_found, storage | — | — |
| `stats.series` | `stats.get`'s range and filter (including the session bounds), `interval?` (`day` default, `week`), `split?` (`block`, `project`, `epic`, `tag`, `item`), `measures: [id]` (required, non-empty: `minutes`, `sessions`, a metric id or `distractions_per_hour`) | `range`, `filter`, `interval`, `split`, `periods`, `series` | validation (empty `measures`, range over 730 days), not_found (unknown measure), storage | — | — |
| `sessions.list` | `stats.get`'s range and project, epic, tag and item filters, `with_note?` (default true), `q?`, `minutes_min?`, `minutes_max?`, `values?: {metric_id: {min?, max?}}`, `limit?` (default 100, 1–500), `before?` (a previous `next`) | `rows: [SessionView + project + epic + item]`, `next: cursor\|null` | validation, not_found, storage | — | — |
| `item.parse` | `text` | draft `{title, block, repeat, weekdays, planned_minutes, start_day}` | validation (and, only when the text names a day, config/source) | — | — |
| `item.preview` | `block`, `repeat`, `weekdays?`, `planned_minutes?`, `date` | `next_occurrence`, `occurrences_this_month`, `schedule_text` | validation | — | — |
| `item.add` | `op_id`, `title` or `quick_text`, `block?`, `repeat?`, `weekdays?`, `project_id?`, `epic_id?`, `planned_minutes?`, `date?`, `metric_ids?`, `tag_ids?`, `backlog?` (one-time, then no `block`/`date`) | mutation result (`changed.items`) | validation, not_found, source, storage | yes | no |
| `item.edit` | `op_id`, `id`, `expected_version?`, `title?`, `block?`, `repeat?`, `weekdays?`, `project_id?`, `epic_id?`, `planned_minutes?`, `tag_ids?`, `metric_ids?` (`null` clears the project, epic, planned minutes or metric selection) | mutation result | validation, not_found, conflict, storage | yes | yes |
| `item.move` | `op_id`, `id`, `block` (required; `null` reorders the backlog), `after_id?` / `before_id?` / `index?`, `expected_version?` | mutation result (every renumbered item) | validation, not_found, conflict, storage | yes | yes |
| `item.schedule` | `op_id`, `id`, `day`, `block`, `expected_version?` (one-time items only) | mutation result (`changed.items`) | validation, not_found, conflict, source, storage | yes | yes |
| `item.backlog` | `op_id`, `id`, `expected_version?` (one-time items only) | mutation result (`changed.items`) | validation, not_found, conflict, source, storage | yes | yes |
| `item.remove` | `op_id`, `id`, `expected_version?` | mutation result (`changed.deleted`) | not_found, conflict, storage | yes | yes |
| `epics.list` | `project_id?`, `status?` (`active` or `finished`) | epics with work totals and task counts, in project and position order | validation, storage | — | — |
| `epic.add` | `op_id`, `project_id`, `name`, `note?` | mutation result (`changed.epics`) | validation, not_found, storage | yes | no |
| `epic.edit` | `op_id`, `id`, `expected_version?`, `name?`, `note?`, `status?` | mutation result (`changed.epics`) | validation, not_found, conflict, storage | yes | yes |
| `epic.remove` | `op_id`, `id`, `expected_version?` | mutation result (`changed.deleted`, detached `changed.items`) | not_found, conflict, storage | yes | yes |
| `projects.list` | `archived?` | `[Project + tags:[Tag] + metrics:[ProjectMetric + metric:Metric]]` | storage | — | — |
| `project.add` | `op_id`, `name`, `color?`, `note?`, `tag_ids?`, `metrics?:[{metric_id, required}]` (defaults to focus, quality and distractions in that order, with `required: false` for distractions) | mutation result (`changed.projects`) | validation, not_found, storage | yes | no |
| `project.edit` | `op_id`, `id`, `expected_version?`, any add field (`note: null` clears), `archived?` | mutation result | validation, not_found, conflict, storage | yes | yes |
| `tags.list` | — | `[Tag]` | storage | — | — |
| `tag.add` | `op_id`, `name`, `color` | mutation result (`changed.tags`) | validation, storage | yes | no |
| `tag.edit` | `op_id`, `id`, `expected_version?`, `name?`, `color?` | mutation result | validation, not_found, conflict, storage | yes | yes |
| `tag.remove` | `op_id`, `id`, `expected_version?` | mutation result (`changed.deleted`, detached `changed.projects`) | not_found, conflict, storage | yes | yes |
| `metrics.list` | `archived?` | `[Metric]`, built-ins first | storage | — | — |
| `metric.add` | `op_id`, `name`, `unit?`, `kind` (`count`/`scale`/`check`), `scale_max?` (scale: 2–10), `rollup?` (default `avg` for a scale, else `sum`) | mutation result (`changed.metrics`) | validation, storage | yes | no |
| `metric.edit` | `op_id`, `id`, `expected_version?`, `name?`, `unit?`, `archived?` (a built-in cannot be archived; `kind` never changes) | mutation result | validation, not_found, conflict, storage | yes | yes |
| `session.start` | `op_id`, `item_id?` / `project_id?` / `epic_id?` | mutation result (`changed.sessions`) | validation, not_found, conflict (one already runs), storage | yes | no |
| `session.stop` | `op_id`, `id?` (defaults to the running timer), `values?:{metric_id: number}`, `note?`, `minutes?`, `done?: bool` | mutation result (`changed.sessions`) | validation, not_found, storage | yes | no |
| `session.bump` | `op_id`, `id?` (defaults to the running timer), `metric_id`, `delta` (1 or −1) | mutation result (`changed.sessions`) | validation, not_found, storage | yes | no |
| `session.note` | `op_id`, `id?` (defaults to the running timer), `text` | mutation result (`changed.sessions`) | validation, not_found, storage | yes | no |
| `session.log` | `op_id`, `item_id?` / `project_id?` / `epic_id?`, `date?`, `block?`, `minutes?` (none: a tick), `values?`, `note?` | mutation result | validation, not_found, storage | yes | no |
| `session.edit` | `op_id`, `id`, `expected_version?`, `minutes?`, `values?`, `note?` (`null` clears), `block?`, `remove_note_ids?: [id]` | mutation result | validation, not_found, conflict, storage | yes | yes |
| `session.remove` | `op_id`, `id`, `expected_version?` | mutation result (`changed.deleted`) | not_found, conflict, storage | yes | yes |
| `prayer.mark` | `op_id`, `prayer`, `date?`, `expected_version?` | mutation result (`changed.marks`) | validation, conflict, storage | yes | optional |
| `prayer.unmark` | same as `prayer.mark` | mutation result (`changed.marks`) | validation, conflict, storage | yes | optional |
| `config.get` | — | `Config` | config | — | — |
| `config.set` | `op_id?`, any `Config` field | `Config` | validation, config | yes with `op_id` | no |
| `subscribe` | `topics?` (`["changes"]`, `["clock"]`, or both), `after_revision?` | `revision`, `after_revision?`; the socket transport then pushes `changed` and/or `clock.boundary` | validation | — | — |

The `entry.*`, `timer.*`, `history.get`, `export`, and `import` methods of the
first planner model are gone and answer `-32601`. The golden responses of every
method live in `crates/salah-cli/tests/fixtures/rpc/` and are the reference
shapes for clients.

A mutation result is `{revision, op_id, changed}`. `changed` always carries
every list — `items` (`ItemView`: the stored item plus `schedule_text` and
`next_occurrence`), `sessions` (`SessionView`), `projects`, `epics`, `resources`, `tags`, `metrics`,
`marks`, and `deleted` (removed ids) — so clients never guard for a missing key.

## Shapes

- Each epic has an `id`, `project_id`, `name`, nullable `note`, `position`, `status`,
  nullable `finished_at`, and the usual sync fields. Finishing sets the timestamp once.
  Reopening clears it. Removal requires zero live sessions, including running timers. It
  soft-deletes the epic and detaches all its items in one revision. Only live items
  appear in `changed.items`. Frozen plans and removed sessions retain their epic.
- `epics.list` includes `total_minutes`, `sessions`, `days_worked`, and `last_worked`
  (latest tracking day, or null). Finished, live sessions supply these totals.
  A tick counts as a session and a day worked. Each live task contributes once to
  `open_tasks` or `done_tasks`. A completion tick marks a one-time task done and
  a routine is done for the tracking day on which it was ticked.
- `SessionView`: `id`, `item_id`, `project_id`, `epic_id`, `day`, `block`, `started_at`,
  `ended_at`, `minutes` (`null` while running and for a tick), `running`, `stale`,
  `note`, `notes: [{id, at, text}]`, `values` (`{metric_id: number}`), and `version`.
  A running session also has `max_minutes`, the largest `session.stop.minutes` the
  server accepts at the request instant: the rounded elapsed minutes up to the planning
  day's Sleep cut-off, at least 1. It appears in `status.get`, day rows and mutation
  results. Finished sessions omit it, as do running sessions whose Sleep instant
  cannot be resolved.
  An item session copies its project and epic at creation. A session logged or started
  against an epic alone takes its project from the epic. Later item edits preserve both
  recorded IDs. Save & Done copies the item’s current project and live epic onto a
  new tick. A removed epic yields a null `epic_id`. Timestamped notes are
  ordered oldest first and `at` is a UTC instant.
- `ItemView` and day rows expose nullable `epic_id`. An assigned epic must belong
  to the item’s project. Changing projects clears the old epic. The same edit can
  select an epic in the new project. Frozen day rows use the planned epic. An epic
  given without a project (on `item.add`, or `item.edit` of an item without one)
  brings its project. A finished epic takes no new or moved-in tasks (`validation`,
  "epic is finished; reopen it to add tasks"); its tasks keep it, and sessions may
  still be logged against it.
- Backlog: `ItemView.backlog` is true when a one-time item has `block` and `day`
  `null` (`next_occurrence` is then `null`). A backlog item is in no day plan,
  snapshot, capacity or fulfilment; work logged on it is unplanned. `item.add
  {backlog: true}` takes no `block` or `date` (with `quick_text` the parsed day and
  block are dropped) and appends to the backlog. `item.schedule` sets a day from
  today on and a block. A change of day or block appends the task to the destination
  block. A frozen destination returns `validation` on `day`, including when a later
  Sleep setting makes that date current again. `item.backlog` clears both and appends
  to the backlog. An item already there keeps its position. A past day that already lists the item keeps it. Both
  reject repeating items (`validation`, field `repeat`). `item.move` with `block:
  null` reorders the backlog; a move between the backlog and a block, `item.edit`
  of a backlog item's `block`, or a repeat on it is a `validation` error.
  `backlog.list` reads its rows and filters from one snapshot. A `project_id` of
  `"none"` selects items with a null stored project ID. `tag_id` filters by the
  task's current live tags.
- A `Resource` carries `id`, `item_id`, `epic_id`, `kind`, `title`, `body`,
  `url`, `remind_at`, `done`, `position`, and sync fields (`created_at`, `updated_at`,
  `deleted_at`, `version`). One parent ID is null. The other names a live item or
  epic when adding or listing resources. The parent and kind are fixed at creation.
  A `note` requires a nonblank `body`, a `link` requires an absolute HTTP(S) `url`,
  and a `reminder` requires an RFC 3339 `remind_at`. New reminders start with
  `done: false`. Editing `done` on a reminder completes or reopens it. Omitted
  edit fields keep their values, and `null` clears optional content. Removal
  soft-deletes the resource.
- Day and backlog rows carry `resources: {notes, links, reminders}`. Each count
  includes live resources directly on the item, with reminders counted while open.
- `status.get.reminders` contains open reminders on live items and epics at or
  before the request instant, oldest first. Each `DueReminder` has `id`,
  `item_id`, `item_title`, `epic_id`, `epic_name`, `title`, `remind_at` and
  `overdue`. The other parent's fields are null. `remind_at` is a UTC instant and
  `overdue` is true when it is strictly earlier than the request instant.
  Fixtures are `resources_list.json`, `resource_add_link.json`,
  `resource_add_reminder.json`, `resource_edit_done.json` and `status_reminders.json`.
- `ItemView` and day rows expose `metric_ids` and the effective `metrics` list.
  `metric_ids: null` inherits the project's active asked metrics. Without a project it
  inherits focus, quality, and distractions. An explicit list selects only those metrics
  and can be empty. Each selected ID must be an active metric the project asks for or a
  built-in. Effective prompts follow project order, with extra selected built-ins
  appended in selection order. Historical rows use the current item's metric selection
  for their session editors.
- Changing an item's project resets its selection to `null`, unless the same
  `item.edit` sends `metric_ids`: that list is then validated against the new project. When a project drops an
  asked metric, the server removes that ID from the project's custom item selections
  in the same write. Updated live items appear in `changed.items`.
- `status.get.prompt` uses the running item's effective `metrics` list, or the active
  project metrics for a project-only timer. Idle status returns an empty list. Each
  `PromptMetric` carries `metric_id`, `name`, `unit`, `kind`, `scale_max`, and
  `required: false`. The `required` field on `ProjectMetric` is retained for compatibility.
  All session metrics and notes are optional.
- Day rows (`day.get` blocks and `status.get` planner): `id`, `title`, `block`,
  `repeat`, `weekdays`, `position`, `version`, `project` (`{id,name,color}` or
  `null`), `planned_minutes` (the day's snapshot), `actual_minutes`,
  `sessions`, `done` (the item has a live tick that day),
  `schedule_text`, `next_occurrence`, and `detail_text` (`"20 / 30 min ·
  Daily"`). Running sessions add no minutes until they stop. A row's `project`
  is the project as planned that day.
- `fulfilment`: `planned_items`, `done_items`, `planned_minutes`,
  `actual_minutes` (only sessions of that day's planned items) and
  `unplanned_minutes` (every other session: unplanned items, bare projects).
  Each day block carries `capacity_minutes`, `planned_minutes`,
  `actual_minutes` and `unplanned_minutes` the same way.
- Item views and day rows expose `tag_ids`. On `item.add`, omitting the field
  copies the project's live defaults, or an empty set for an item without a
  project. An explicit list replaces those defaults, including `[]`. An
  `item.edit` with `tag_ids` replaces the set. Project changes preserve existing
  task tags. Both methods validate each id against live tags. Removing a tag
  detaches it from tasks and projects and bumps the affected versions.
- All session reports resolve tags at read time using the task's current tags,
  or the recorded project's current defaults for sessions without a task.
  Re-tagging a task therefore updates its historical reports.
- `stats.table` is the general statistics table. It reads finished live sessions only
  (ticks included) and never writes. Filters intersect; `"none"` selects unassigned
  work: no project, no live session tag, or no task. A named id must exist (a removed
  item still matches its sessions). `columns` lists `minutes`, `sessions`, then every
  active metric and each archived metric with a value in scope. The order follows the
  catalog, or the project's prompt order under a single `project_id`. Each column is
  `{id, name, unit, kind, rollup, scale_max, archived, derived_from}`. After
  `distractions` comes the derived `distractions_per_hour` (`derived_from:
  "distractions"`, unit `per hour`): the row's distraction sum times 60 over its
  minutes, `null` without a sample or without minutes. Rows are `{key, label, color,
  from, to, minutes, sessions, values}` and `values` has a key per metric column
  (`null` without a sample). `avg` metrics are weighted by session minutes, a tick
  weighing 1; `sum` metrics add up.
  - `week`: every Monday-first week touching the range, oldest first, keyed by ISO
    week (`2031-W20`) with `from`/`to` clipped to the range, empty weeks included
    with zeros and nulls. `day`: every day, keyed by date.
  - `project`, `epic`, `tag`, `item`: only keys with sessions, most minutes first; `from`/`to`
    are `null`. Unassigned work is the row `key: "none"` ("No project", "No epic", "No tag",
    "No task", `color: null`), so the rows cover every session in scope. A tag row
    counts each session carrying that tag; a session counts once per tag,
    and `totals` count each session once. Task and epic rows are coloured by the session's
    project. A row `key` is a valid filter value for its dimension.
  - `block`: rows with work or ticks, in prayer order. A timed session's recorded
    minutes are apportioned across the blocks it overlapped, proportional to elapsed
    time in each block. Remaining whole minutes go to the largest fractional
    remainders, with the starting block first on ties. This also applies to a Worked
    override. A manual log stays in its recorded block. A tick contributes zero
    minutes and one metric sample in its recorded block. Each block with assigned
    minutes counts the session once. Averages use those minutes as weights, and
    sums use that block's share of recorded minutes. Day block bars use the same
    allocation for actual and unplanned minutes. Each session keeps one row, its
    original block, and its full duration on the task and day. A row `key` is the
    block id (`fajr` … `isha`, or `night` for work before Fajr), `label` its name,
    `color`/`from`/`to` `null`. A past day uses its frozen block boundaries; other
    days use the timetable of the config in force that day. In `stats.get`, each
    entry of `blocks` uses this allocation for `minutes` and this weighting for
    `avg_focus`. All six blocks appear in prayer order with their capacity over
    the range. Empty blocks have zero minutes and null average focus.
  - `totals`: `{minutes, sessions, values}` over every session in scope.
- `stats.series` feeds the Trends line chart and aggregates exactly like `stats.table`
  (same filters, minute weighting, rollups, `distractions_per_hour`, `key: "none"`). One
  series per (split key x measure), in split-key order then `measures` order (duplicate
  ids are dropped). `periods` lists each day, or each Monday-first week's first day
  (the first week clipped to `from`). A series is `{key, label, color, measure, unit,
  scale_max, points: [{value, sessions}]}` with one point per period. `value` is `null`
  for a period without data, except `minutes` and `sessions`, which are 0; `sessions`
  counts the sessions behind the point. Without `split`, `key` is `"all"`, `label` the
  measure's name and `color` `null`. A block split uses the table's block keys, order and
  allocation. For other splits, `key` is the project, epic, tag or item
  id (`"none"` for unassigned work), `color` its colour, and `label` the key's label
  (`key label · measure name` when several measures). At most 8 keys are kept, most
  minutes first; further keys merge into one `key: "other"`, `label: "Other"` series
  that counts each session once. A tag split counts a session in each of its tags.
- Session bounds (`stats.get`, `stats.table`, `sessions.list`): `minutes_min?`,
  `minutes_max?` and `values?` share one predicate. `minutes_*` are inclusive and never
  match a tick; each `values` entry needs the session to have that metric within the
  inclusive bounds (a session lacking it does not match); an unknown metric is
  `not_found`; reversed bounds or a negative duration are `validation` errors naming
  the field. On `stats.get` and `stats.table` they narrow which sessions are
  aggregated (days, series, projects, tags, blocks, totals, rows). Planned minutes,
  capacity and the fulfilment denominator are unaffected. The result's `filter` echoes
  `minutes_min`, `minutes_max` (`null` when absent) and `values` (`{}` when absent)
  beside `tag_id`, `project_id`, `epic_id` and `item_id`. `stats.get` also accepts `"none"` for
  the project, epic, tag and item filters, as `stats.table` does; under `item_id: "none"`
  or a project/epic/tag filter the plan lines narrow the same way.
- `sessions.list` is the Notes page query. Every predicate, the cursor, and the limit
  run in SQL. Rows are finished live sessions (ticks included): newest planning day
  first, then the newest end instant (creation for manual logs and ticks), then id.
  Each row is a `SessionView` (with its timestamped `notes`) plus `project:
  {id,name,color}|null`, `epic: {id,name}|null` and `item: {id,title}|null`, the
  session's recorded project, epic and task with their current labels. `with_note` keeps sessions with a main note or
  a live timestamped entry. `q` is trimmed and matches either case-insensitively
  (Unicode lowercase, literal text). `minutes_min`/`minutes_max` are inclusive and
  never match a tick. Each `values` entry needs the session to have that value within
  the inclusive bounds; an unknown metric is `not_found`. `next` is
  `"<day>|<instant>|<id>"` of the page's last row when more rows follow; pass it back
  as `before`. Reversed bounds, a negative duration, a limit outside 1–500 or a
  malformed cursor are `validation` errors naming the field.
- `stats.get` accepts an inclusive range of 1–730 planning days. The item, epic, project, and
  tag filters intersect. Only finished sessions count. The item filter uses the session's
  recorded item ID, including historical sessions for removed items. It also narrows
  planned minutes and fulfilment to that item. An `avg` rollup is weighted by
  session minutes, a tick weighing 1. Each `days[]` row is `{day, minutes,
  capacity_minutes, planned_minutes, unplanned_minutes, fulfilment_ratio}`;
  `fulfilment_ratio` is plan-line minutes over planned minutes, capped at 1,
  and `null` when nothing was planned. A day's `capacity_minutes` leaves out
  the night block (sleep to Fajr); per-block capacity still lists it.
- `stats.get.projects` lists each project with work in scope, in project order, then
  one `{id: "none", name: "No project", color: null}` row when sessions without a
  project are in scope, so the rows add up to `totals`.
- `stats.get.tags` counts a session’s minutes in each of its current tags. A session with an empty tag set contributes
  to the final `{id: "none", name: "No tag", color: null, minutes}` row. An
  untagged tick contributes zero minutes. See `stats_with_untagged_tick.json`.
- `stats.get.epics` contains `{id, name, project_id, minutes, sessions, metrics}`
  for each epic with work in scope. Within each project, work without an epic
  has `id: "none"` and the name "No epic". Work without a project has a null
  `project_id` and also contributes to the "No project" project row. Rows follow
  project order (work without a project last); within a project its epics come
  most minutes first and its "No epic" row last. The epic table and trend split
  combine unassigned sessions under the key `"none"`.
- `item.parse` also reads "today" and "tomorrow" (any case, trailing punctuation
  ignored; the first one counts) and drops the word from the title. `start_day` is
  that tracking day (`YYYY-MM-DD`, the Sleep boundary decides "today"), or `null`
  when the text names no day. `item.add` with `quick_text` uses it as the item's
  `date` unless `date` is given.
- Snapshots: days from today on are computed live from the current config and
  items and are never stored. Once a day is past, its blocks and plan are
  written once (from the items that existed by its end, with their project
  then) and marked written, so an empty plan stays empty. A past day's prayer
  times and sleep time come from its stored blocks. An item write freezes
  unwritten days from the earliest item start date through yesterday, up to
  the preceding 366 days, in the same transaction as the item change.
  Reads of older unwritten days use the recorded item and configuration
  history. Snapshot writes leave the revision and `changed` list untouched.
- `item.add` rejects a `date` before today.
- A bare `session.stop` pauses the timer and records its elapsed minutes. `session.log`
  with minutes records timed work. Either operation accepts an empty or partial values map.
  Supplied values must pass the metric's kind and range checks. Archived metrics are
  omitted from prompts and reject new values. Edits retain existing archived samples.
- `session.stop.minutes` overrides the recorded duration. It must be an integer between
  1 and the rounded, clamped elapsed minutes. The recorded end instant uses the ordinary
  timer cutoff. With `done: true`, the server also ticks the item on the timer's planning
  day and returns the stopped timer followed by the tick in `changed.sessions`. The timed
  session receives the supplied values and note. The server creates an empty tick or
  reuses an existing live tick on the same day. The stop and new tick share one revision
  and `op_id`. A project-only timer rejects `done: true` with `validation`.
- `session.bump` adjusts an active count metric on the running timer, flooring the value at
  zero. `distractions` is a built-in count with sum rollup and unit `times`. It can be
  bumped regardless of the prompt. When stopping, supplied values override the corresponding
  live values and omitted values retain them.
- `session.bump` and `session.note` refuse a stale timer (the request is past its
  planning day's Sleep instant) with `validation` on `id`: stop it first.
- `session.note` appends a separate timestamped entry to the running timer. Trimmed text
  must contain 1–500 characters. `session.edit.note` edits the main note.
  `session.edit.remove_note_ids` soft-deletes the named entries belonging to the session.
  Removing a session soft-deletes all of its live entries. Note writes advance the
  session version and use the `sessions` notification scope.
- To mark an item done, call `session.log` with its `item_id` and omit `minutes`.
  If that item has a running timer, the server stops it at the usual clamp point first
  and writes the tick in the same transaction. The tick receives the supplied metrics
  and note. The timer receives no new values. The timer keeps its original day and
  the tick uses the log's day. Both writes share one revision and `op_id`.
  `changed.sessions` returns the new tick first, followed by the stopped timer.
- `fulfilment.done_items` counts planned items with at least one live tick. Removing
  the last tick for an item with `session.remove` clears its done state for that day.
  Its timed sessions still count toward `actual_minutes` and `fulfilment_ratio`.
- An open timer has `running: true` until it is stopped. Its `stale` flag becomes
  true when the request time is strictly past its planning day's Sleep instant.
  Stopping a stale timer sets `ended_at` to that Sleep instant and records the
  clamped minutes (at most 1440). Finished sessions and ticks have `stale: false`.
  If status can read the timer but cannot resolve its Sleep instant, it keeps the
  running session, omits `stale`, and reports the failure in `planner_error`.
  Earlier v1 responses, including stored mutation replays, can also omit `stale`.

`now.next.at` in `day.get` and `next.at` in `status.get` are UTC instants. A
client computes a countdown as `next.at - Date.now()`; it does not add local
boundary maths.

`item.preview` is read-only and needs no `op_id`. It validates the draft like
`item.add` (`block` may not be `sleep`, a weekly draft needs weekdays) and
returns the first occurrence on or after `date`, the number of remaining
occurrences in that calendar month, and `schedule_text`. It never reads or
writes the store.

`item.move` accepts `after_id` or `before_id` and resolves them inside the
write transaction. Clients should send a neighbour id from the rendered row
list.

## Errors

Application errors use fixed `-32000-n` codes and carry stable
`error.data.kind` values taken directly from `salah_domain::Error::code()`:

| Kind | Code | Retryable |
|---|---|---|
| `validation` | -32001 | no |
| `not_found` | -32002 | no |
| `ambiguous` | -32003 | no |
| `conflict` | -32004 | no |
| `busy` | -32005 | yes |
| `storage` | -32006 | no |
| `source_unavailable` | -32007 | yes |
| `config` | -32008 | no |
| `internal` | -32009 | no |

The top-level `message` is human text; clients branch on `data.kind`, not on
the message. Validation errors may add `data.field`. Protocol faults use the
JSON-RPC reserved codes: `-32700` parse, `-32600` invalid request, `-32601`
method not found, `-32602` invalid params, `-32603` internal serialization.
Protocol faults have no application `kind` (invalid params does carry
`kind:"validation"` so callers have one branch point).

Example stale edit:

```json
{"jsonrpc":"2.0","id":9,"error":{"code":-32004,
 "message":"item \"…\" is at version 4, not 3",
 "data":{"kind":"conflict","retryable":false}}}
```

## Idempotency and versions

- A repeated `op_id` returns the stored result and revision and performs no
  write, even if the retry body differs. An `op_id` belongs to the method it
  was first used with: reusing it for another method is `validation`
  (`data.field = "op_id"`), and an empty `op_id` is rejected the same way. On first application the dispatcher
  records the complete serialized protocol result in `applied_ops` inside the
  same `BEGIN IMMEDIATE` transaction as the mutation. A replay returns those
  exact bytes, including the `changed` rows as they were then, even if later
  edits have changed the current state.
- `item.move` resolves `after_id`/`before_id` against the siblings read inside
  the same write transaction that reorders the block, so the anchor cannot go
  stale between the read and the move.
- `config.set` cannot share SQLite's transaction with the JSON config file. It
  writes the config file first through the repository's atomic rename and then
  records `op_id` in the same SQLite write transaction as `applied_ops`, so a
  replay returns the recorded `Config` without touching the file. If the
  process dies between the rename and the commit, the new file remains without
  an op row and a retry applies its body once more; the file is never left
  half-written.
- Reuse the same `op_id` when retrying after a timeout; use a new one for a new
  intent.
- `expected_version` applies per entity on every `*.edit` and `*.remove`,
  `item.move`, `item.schedule`, `item.backlog`, and optionally prayer marks. Appends (`*.add`, `session.start`,
  `session.stop`, `session.bump`, `session.note`, `session.log`) need no precondition, only `op_id`. A stale
  version is `data.kind = "conflict"` and writes nothing.
- Every entity row carries its `version` (lists, day rows, and `changed`). `revision` orders pushes and de-duplicates them; it
  is never sent as a mutation precondition.

## Notifications

A client subscribes after `hello`:

```json
→ {"jsonrpc":"2.0","id":4,"method":"subscribe",
   "params":{"topics":["changes"],"after_revision":184}}
← {"jsonrpc":"2.0","id":4,"result":{"revision":184}}
```

The stdio transport accepts `subscribe` and returns the revision but has no
push channel. The socket daemon registers the connection and pushes every
revision advance, including those caused by a direct `salah` CLI write to the
same database.

Notifications have no `id`:

```json
{"jsonrpc":"2.0","method":"changed",
 "params":{"revision":185,"days":["2031-05-14"],
           "scopes":["items","sessions"],"op_id":"…","actor":"widget"}}
```

`changed` is an invalidation, not a patch: refetch `status.get` or `day.get`.
`days` and `op_id` may be omitted; each scope is one of `items`, `sessions`,
`projects`, `epics`, `resources`, `tags`, `metrics`, `marks`, `config`. The daemon emits these from its `data_version` watcher; a
notification may cover more than one commit observed in the same poll, so
clients treat its `revision` as the newest state to fetch and de-duplicate by
revision.

The daemon also pushes `clock.boundary` when the current block or next boundary
changes, and once as an immediate snapshot after `subscribe` with the `clock`
topic:

```json
{"jsonrpc":"2.0","method":"clock.boundary",
 "params":{"day":"2031-05-14","block":"fajr",
           "now":"2031-05-14T04:30:00Z",
           "next":{"id":"dhuhr","name":"Dhuhr","time":"12:19",
                   "at":"2031-05-14T07:19:00Z","remaining_seconds":10140}}}
```

`now` and `next.at` are the authoritative instants. `remaining_seconds` is a
convenience snapshot at that instant; clients should count down from `next.at`
rather than assuming the notification arrives at `now`.

## Evolution rules

- Protocol `1` is additive: new optional fields, new methods, and new
  capability strings may appear. Clients ignore unknown fields and methods
  they do not call.
- Existing field names, types, meanings, method names, and error `kind`
  strings do not change within major `1`.
- A breaking change needs a new method name or a new major protocol version;
  the server may serve several majors.
- Enum values (`prayer`, `block`, `repeat`, `measure`, `state`, `kind`) are
  part of the contract. Additions require a contract review.
- The CLI v1 fixtures remain the compatibility suite for the human/script
  interface; they do not constrain the RPC wire format.
