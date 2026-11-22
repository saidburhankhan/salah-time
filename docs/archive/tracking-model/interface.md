# tracking-model interface (locked by orchestrator, 2026-11-22)

Backend (W1) produces this contract. Frontends (W2) consume it. A change to this
file is made by the orchestrator only; workers request changes in their report.

## Domain types (`salah-domain`)

Keep: `Repeat`, `Weekdays`, `PrayerMark`, `BlockKey`, `BoundaryId`,
`validate_title`, `validate_block`, sync fields (`created_at`, `updated_at`,
`deleted_at`, `version`) on every synced type.

Remove: `Measure`, `EntryKind`, `Entry`, and item `measure`/`target`.

```rust
Project       { id, name, color: String, note: Option<String>, archived: bool, tag_ids: Vec<String> }
Tag           { id, name, color }
MetricKind    = Count | Scale | Check        // serde lowercase
Rollup        = Sum | Avg
Metric        { id, name, unit: String, kind, scale_max: Option<i64>, rollup, builtin: bool, archived: bool }
ProjectMetric { project_id, metric_id, required: bool, position: i64 }
Item          { id, title, project_id: Option<String>, block, repeat, weekdays, day /*start*/,
                planned_minutes: Option<i64>, position, .. }
Session       { id, item_id: Option<String>, project_id: Option<String>, block: BlockKey,
                started_at: Option<DateTime>, ended_at: Option<DateTime>,
                minutes: Option<i64>, note: Option<String>, values: Vec<SessionValue>, .. }
SessionValue  { metric_id, value: f64 }
DayBlock      { day, block: BoundaryId, starts_at, ends_at, source: Official | Calculated }
DayPlanLine   { day, block: BoundaryId, item_id, planned_minutes: Option<i64> }
```

Rules:
- A session has `item_id` or `project_id` (or both). When it has an item, `project_id` is
  copied from the item when the session is written.
- A running timer has `started_at` set and `ended_at = None`. On stop, `minutes` is
  computed from the times (rounded, at least 1). Only one session can be running at a time.
- A session with no `minutes` is a tick.
- Values are validated against the metric: a scale must be an integer in 1..=scale_max,
  a count must be >= 0, and a check must be 0 or 1. An archived metric is rejected for new values.
- The required metrics of the session's project must be present when a session is
  stopped or logged; otherwise `validation` with `data.missing = [metric_id]`.
  A timer start never requires values.
- Built-in metrics are seeded with fixed ids `focus` (scale 5, avg) and
  `quality` (scale 5, avg). A new project gets both as required, unless the
  caller passes `metrics`.
- `DayBlock`/`DayPlanLine` for a day are materialized the first time the day is read
  or written. Today's plan follows item edits; past days never change.

## Store (`salah-store`)

One migration, schema v1, as in `plan.md` (`PRAGMA foreign_keys = ON`, enum
`CHECK`s, indexes). The old migration chain is deleted; an existing database with
`user_version` greater than 0 from the old schema is renamed to `salah.db.old-<ts>`
and a fresh database is created (there is no user data). Keep `meta`, `changes`,
`applied_ops` and the write/op-id machinery unchanged.

## Protocol changes (`salah-protocol`, JSON-RPC over the existing transport)

Removed: `entry.done`, `entry.undo`, `entry.log`, `timer.start`, `timer.stop`,
`export`, `import` (export/import is already deferred). `item.preview` loses `measure`/`target`.

Changed:
- `item.add` / `item.edit`: `measure` and `target` are replaced by `project_id?` and
  `planned_minutes?` (`null` clears).
- `day.get` row (see Review decisions for fulfilment): `project` `{id,name,color}|null`, `planned_minutes`,
  `actual_minutes`, `sessions: [SessionView]`, `done: bool`. Each block gets
  `capacity_minutes`, `planned_minutes`, `actual_minutes`. The day gets
  `fulfilment {planned_items, done_items, planned_minutes, actual_minutes}`.
- `status.get` adds `running: SessionView|null` and `prompt: [PromptMetric]`.
  `prompt` lists the metrics to ask when the running session stops:
  `{metric_id, name, unit, kind, scale_max, required}`.
- `stats.get` params: `from,to` or `days?`, plus optional `tag_id`, `project_id`.
  The result:
  ```
  { range, filter,
    days:   [{ day, minutes, capacity_minutes, planned_minutes, fulfilment_ratio }],
    series: [{ metric_id, name, unit, rollup, points: [{day, value|null}] }],   // minutes first
    projects: [{ id, name, color, minutes, sessions, metrics: {metric_id: value} }],
    tags:     [{ id, name, color, minutes }],
    blocks:   [{ block, minutes, capacity_minutes, avg_focus|null }],
    totals:   { minutes, sessions, active_days, avg_focus|null, avg_quality|null } }
  ```
  An `avg` rollup is weighted by session minutes (ticks weigh 1).

Added (all mutations take `op_id`; edits take `expected_version?`):
| Method | Params | Result |
|---|---|---|
| `projects.list` | `archived?` | `[Project + tags + metrics:[ProjectMetric+Metric]]` |
| `project.add` | `name`, `color?`, `note?`, `tag_ids?`, `metrics?:[{metric_id, required}]` | mutation result |
| `project.edit` | `id`, any add field, `archived?` | mutation result |
| `tags.list` / `tag.add` / `tag.edit` / `tag.remove` | `name`, `color` / `id` | list / mutation result |
| `metrics.list` | `archived?` | `[Metric]` |
| `metric.add` | `name`, `unit`, `kind`, `scale_max?`, `rollup?` | mutation result |
| `metric.edit` | `id`, `name?`, `unit?`, `archived?` (built-ins: no `kind` change, no archive) | mutation result |
| `session.start` | `item_id?` / `project_id?` | mutation result |
| `session.stop` | `id?` (default: the running one), `values?:{metric_id: number}`, `note?` | mutation result |
| `session.log` | `item_id?` / `project_id?`, `date?`, `block?`, `minutes?`, `values?`, `note?` | mutation result |
| `session.edit` | `id`, `minutes?`, `values?`, `note?`, `block?` | mutation result |
| `session.remove` | `id` | mutation result |

`changed` notifications gain the entity names `projects`, `tags`, `metrics`, `sessions`.
The contract fixtures under `crates/salah-cli/tests/fixtures/rpc/` are regenerated and
are the source of truth for the frontends.

## UX decisions (S0, locked)

- **Widget (recording):** the bar shows the next prayer and, when a session is
  running, the project colour dot and elapsed time. The popup lists today's blocks with
  their items: tapping an item starts or stops its timer. **Stop opens an inline
  panel** that asks for the `prompt` metrics: scales as 1–5 tap buttons, counts as a
  number field, checks as a toggle, plus an optional one-line note. One "Save" sends
  `session.stop`. A "+ Log" button opens project + minutes + the same metrics
  (`session.log`). Ticking an item without minutes is `session.log` with no
  minutes.
- **App, Today:** blocks between prayers, each with a capacity bar (planned vs
  available vs actual). Plan lines show the project colour, planned minutes and
  their sessions. A day fulfilment summary is at the top.
- **App, Settings:** pages for Projects (name, colour, tags, metrics asked and
  whether each is required), Tags, and Metrics.
- **App, Stats:** a filter bar (All / tag / project) and a range (7 / 28 / 90 days).
  It shows KPI tiles (minutes, active days, avg focus, fulfilment), a line chart
  per metric series, minutes per day against capacity, a project breakdown and a
  block breakdown.

## Review decisions (orchestrator, 2026-11-22 22:5x)

- **Day capacity** excludes the Night block (sleep→Fajr). Per-block capacity still lists Night.
- **Fulfilment** counts only minutes logged against that day's plan lines (`actual_minutes`).
  Sessions without a plan line are reported separately as `unplanned_minutes` on the day,
  on each block, and in `stats.days[]`. `fulfilment_ratio` = plan-line minutes / planned minutes,
  capped at 1, or `null` when nothing was planned.
- **Snapshots:** a day's plan and blocks are frozen once the day is past. "Written" is
  marked explicitly, so an empty plan is not the same as a missing one. A past day that was never
  materialised is built once from the items that existed then (`created_at` ≤ the day's end) and the
  item's project at that time, then frozen. `day_plan` carries `project_id`. Days ≥ today are
  computed live from the current config/items, never stored ahead. The prayer times shown for a past day
  come from its stored `day_blocks`.
- **Items** cannot start before today (no back-dated plans).
- **Archived metrics** are no longer required or prompted. Existing values of an archived
  metric stay valid on `session.edit`; new values for it are rejected.
- **Timer minutes** are clamped to the end of the session's day (the Sleep boundary), at most 1440.
- **Stats** count only finished sessions in `sessions` totals.
- **op_id:** an empty op_id is rejected; a replay with a different method is a `validation` error.
