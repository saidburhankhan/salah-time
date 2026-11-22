# Plan: tracking-model

## Outcome

Salah becomes a day-centred tracker for learning and work. Each day is planned
in the prayer blocks and fulfilled by sessions. Each session records minutes plus
the metrics its project needs (focus, quality, and user-defined outputs such
as pages). Projects and tags exist for grouping and analytics: the whole
picture is read by tag → project → metric, as lines over time, against the
time that each day actually had.

## Decisions

- **The day is the primary unit.** Planning and the widget work on today. A day
  is "fulfilled" by comparing that day's plan snapshot with its sessions.
- **Projects and tags are only for analysis.** No project workflow, no
  tasks inside projects. Tags group projects (e.g. `learning`, `work`,
  `rust`); there is no separate project "kind".
- **Start fresh.** There is no user data. Drop the old `items`/`entries` schema and
  the old migration chain and write a single clean v1 schema. No data migration,
  no compatibility with old exports.
- **Minutes is a column; everything else is a metric.** Focus and quality are
  seeded built-in scale metrics. User metrics are added from settings as rows,
  never by migration.
- **Each project decides what the widget asks.** `project_metrics` lists the
  metrics to prompt for after a session, and which of them are required.
- **History is frozen.** Each day's block times (`day_blocks`) and plan
  (`day_plan`) are snapshotted, so later edits, location changes, or method
  changes never rewrite past days.
- **Recording must stay quick.** Only metrics that are cheap to enter and give
  direction at a glance are included. No elaborate measurement tools.
- Keep the existing sync columns (`created_at`, `updated_at`, `deleted_at`,
  `version`), the change log, and applied operations, because the daemon depends on them.

## Schema v1 (draft)

Every synced table also has `created_at`, `updated_at`, `deleted_at`, `version`.
Enums are `TEXT` with `CHECK`; references use `REFERENCES` with foreign keys
on. Rows are deleted only by tombstone.

```sql
projects        (id, name, color, note NULL, archived INTEGER)
tags            (id, name UNIQUE, color)
project_tags    (project_id → projects, tag_id → tags, PK(project_id, tag_id))

metrics         (id, name, unit,
                 kind   CHECK IN ('count','scale','check'),
                 scale_max NULL,
                 rollup CHECK IN ('sum','avg'),
                 builtin INTEGER, archived INTEGER)
                -- seeded: focus (scale 1–5, avg), quality (scale 1–5, avg)
project_metrics (project_id → projects, metric_id → metrics,
                 required INTEGER, position, PK(project_id, metric_id))

items           (id, title, project_id NULL → projects, block,
                 repeat CHECK IN ('none','daily','weekly'), weekdays NULL,
                 start_day, planned_minutes NULL, position)
                -- a plan line; no project = routine (e.g. Quran after Fajr)

sessions        (id, item_id NULL → items, project_id NULL → projects,
                 day, block, started_at NULL, ended_at NULL,
                 minutes NULL, note NULL,
                 CHECK (item_id IS NOT NULL OR project_id IS NOT NULL))
                -- project_id is copied from the item when written (analytics snapshot)
                -- no minutes = a plain tick; ended_at NULL with started_at = running timer
session_values  (session_id → sessions, metric_id → metrics, value REAL,
                 PK(session_id, metric_id))
                -- written atomically with the session; one change-log entity

day_blocks      (day, block, starts_at, ends_at,
                 source CHECK IN ('official','calculated'), PK(day, block))
day_plan        (day, block, item_id → items, planned_minutes NULL,
                 PK(day, block, item_id))
                -- today's snapshot follows edits; past days are frozen

prayer_marks, meta, changes, applied_ops   -- as today
```

Indexes: `sessions(day)`, `sessions(project_id, day)`, `sessions(item_id)`,
`session_values(metric_id)`, `items(project_id)`.

## Open questions

- Project goals (e.g. "300 pages total"): left out of v1 because the day is the
  focus. Add a `project_goals` table later if needed.
- Tags on sessions: not in v1; tags apply to projects only.
- When `day_plan`/`day_blocks` are materialized: proposed at first daemon access
  to a day and on rollover.

## Non-goals

- Importing old data or old export formats.
- Rich measurement tools, project task management, or a cloud sync backend.

## Slices

- [x] S0 — Schema v1 and the app/widget/stats design are locked in `interface.md`.

Owners: **W1 backend** (Claude session `06b42b65`) owns S1–S4: `crates/**`,
the RPC fixtures, and `docs/work/architecture/protocol-v1.md`. **W2 frontends** (Codex,
from 19:37) owns S5: `app/**`, `plugin/**`. The orchestrator owns `docs/work/tracking-model/`,
`docs/START.md`, and integration.
- [ ] S1 — Domain types: Project, Tag, Metric, Item, Session, values, DayPlan.
- [ ] S2 — Store: single v1 schema, constraints, indexes, repository API.
- [ ] S3 — Core: planning, session logging with metric validation, day
      fulfilment, snapshots, stats queries.
- [ ] S4 — Protocol/daemon methods and fixtures.
- [ ] S5 — Widget quick-log flow; app planning and settings (metrics, projects,
      tags); statistics views.
- [ ] S6 — Integration and live verification.

## Acceptance checks

- [ ] A session with minutes, focus, quality and a custom metric is recorded in
      one operation and read back intact.
- [ ] Required project metrics are enforced when a session is logged.
- [ ] Adding a metric from settings needs no schema change.
- [ ] Past days' block times and plans are unchanged after editing an item or
      changing location.
- [ ] Stats can be grouped by tag, project, block and day from SQL aggregates.
- [ ] `./scripts/test` and strict Clippy pass.
