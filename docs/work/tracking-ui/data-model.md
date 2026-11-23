# Data model for phases 2 and 3 (locked by the orchestrator, 2026-11-23)

This file is the single source of truth for every schema change in `tracking-ui`. Workers
implement it exactly; a change needs the orchestrator. `plan.md` describes behaviour; where it
and this file disagree about storage, this file wins.

## Principles

1. **The user has live data.** Every change is a numbered, transactional migration applied in
   place. Nothing is dropped, renamed aside or rewritten lossy. Before migrating a file, the
   store copies it to `salah.db.bak-v<from>-<timestamp>` (one copy per version step; keep the
   last three).
2. **Additive protocol.** Existing methods, results and fixtures keep their shapes; new data is
   new fields, methods or entities. An existing fixture may only gain fields.
3. **Normalised, no placeholders.** No JSON blobs for relations (only the existing history
   snapshots), no fake values standing in for "none" (use NULL plus CHECKs), one fact in one place.
4. **Same layering as today.** Domain types in `salah-domain` (one module per entity when a file
   would pass ~800 lines), tables and queries in `salah-store`, behaviour in `salah-core` modules
   beside `projects`/`sessions`/`stats`, methods in the protocol method table, CLI subcommands
   like `project`. Every write goes through the existing write/op_id/`changes` machinery and
   bumps `version`; every synced table has `created_at, updated_at, deleted_at, version`.
5. **Sessions are the facts.** Stats, notes and epics read sessions (and their values/notes).
   `project_id` and `epic_id` are copied onto a session when it is written and never rewritten by
   later item edits.

## Migration chain

- Migrations live in `crates/salah-store/src/migrations/NNN_name.sql` and run in order by
  `PRAGMA user_version`, each inside one transaction together with its `user_version` bump.
- `001_schema.sql` = today's `schema.sql` (v1). `002_history.sql` = today's `history.sql`
  (`day_clocks`, `item_history`, `config_history`); it stops being applied on every open.
  A database already at v1 with the history tables present migrates to 2 as a no-op.
- A fresh database runs every migration. An unknown newer `user_version` is refused (no downgrade).
- Tests: for every step N, build a database at N−1 with realistic rows (projects, tags, metrics,
  items of each repeat kind, running/finished/tick sessions with values, frozen days, marks), run
  the migration, assert every row and relation survived and `PRAGMA foreign_key_check` and
  `integrity_check` are clean.
- The orchestrator also migrates a copy of the live database in a scratch environment before any
  install.

## 003 — phase 2 (worker C2)

```sql
-- Built-in distraction counter.
INSERT INTO metrics (id, name, unit, kind, scale_max, rollup, builtin, archived,
                     created_at, updated_at, deleted_at, version)
VALUES ('distractions', 'Distractions', 'times', 'count', NULL, 'sum', 1, 0,
        '1970-01-01T00:00:00.000Z', '1970-01-01T00:00:00.000Z', NULL, 1);
-- Every existing project asks it (not required), after its current metrics.
INSERT INTO project_metrics (project_id, metric_id, required, position)
SELECT p.id, 'distractions', 0,
       COALESCE((SELECT MAX(position) + 1 FROM project_metrics pm WHERE pm.project_id = p.id), 0)
FROM projects p WHERE p.deleted_at IS NULL;

-- Per-task metric selection. metrics_custom = 0: the item asks its project's metrics
-- (metric_ids = null in the protocol); 1: exactly its item_metrics rows (possibly none).
ALTER TABLE items ADD COLUMN metrics_custom INTEGER NOT NULL DEFAULT 0
    CHECK (metrics_custom IN (0, 1));
CREATE TABLE item_metrics (
    item_id    TEXT NOT NULL REFERENCES items (id),
    metric_id  TEXT NOT NULL REFERENCES metrics (id),
    position   INTEGER NOT NULL,
    PRIMARY KEY (item_id, metric_id)
);

-- Timestamped notes written while a session runs (session.note). The session's own
-- `note` column stays the main note written at pause/done/edit.
CREATE TABLE session_notes (
    id          TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    at          TEXT NOT NULL,
    text        TEXT NOT NULL CHECK (length(text) BETWEEN 1 AND 500),
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    version     INTEGER NOT NULL CHECK (version >= 1)
);
CREATE INDEX session_notes_by_session ON session_notes (session_id, at);
CREATE INDEX sessions_by_item_day ON sessions (item_id, day);
```

Protocol consequences (these replace the earlier wording in plan.md):
- `SessionView` gains `notes: [{id, at, text}]` (oldest first). `session.note {id?, text, op_id}`
  inserts a `session_notes` row; it never edits `note` or earlier rows. Removing a session
  removes its notes (soft delete with the session). `session.edit` can remove one entry with
  `remove_note_ids?: [id]`.
- `sessions.list` text search `q` matches `note` and every entry; `with_note` means "has a note
  or at least one entry".
- Item views expose `metric_ids` (`null` when `metrics_custom = 0`) and the effective `metrics`.
  When a project stops asking a metric, matching `item_metrics` rows are deleted in the same write;
  when an item's project changes, `metrics_custom` resets to 0 and its rows are deleted.
- `session.bump` writes `session_values` (insert or update), never below 0.

## 004 — phase 3 (worker C5)

`items` needs nullable `block`/`start_day` (backlog) and `epic_id`, and its CHECKs change, so it
is rebuilt with SQLite's documented procedure: `PRAGMA foreign_keys=OFF`, `BEGIN`, create
`items_new` with the new definition, `INSERT … SELECT` every column, drop `items`, rename, recreate
its indexes, `PRAGMA foreign_key_check` (must be empty), `COMMIT`, `PRAGMA foreign_keys=ON`.
Tables referencing `items` (`sessions`, `day_plan`, `item_metrics`, `item_history`, `resources`)
keep working because the name and ids are unchanged; the migration test proves it.

```sql
CREATE TABLE epics (
    id           TEXT PRIMARY KEY,
    project_id   TEXT NOT NULL REFERENCES projects (id),
    name         TEXT NOT NULL,
    note         TEXT,
    status       TEXT NOT NULL CHECK (status IN ('active', 'finished')),
    finished_at  TEXT,
    position     INTEGER NOT NULL,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT,
    version      INTEGER NOT NULL CHECK (version >= 1),
    CHECK ((status = 'finished') = (finished_at IS NOT NULL))
);
CREATE INDEX epics_by_project ON epics (project_id, position);

-- items, rebuilt (new or changed parts only shown):
--   block      TEXT CHECK (block IS NULL OR block IN ('night','fajr','dhuhr','asr','maghrib','isha')),
--   start_day  TEXT,
--   epic_id    TEXT REFERENCES epics (id),
--   metrics_custom … (from 003)
--   CHECK ((block IS NULL) = (start_day IS NULL)),
--   CHECK (block IS NOT NULL OR repeat = 'none'),          -- only one-time tasks can be in the backlog
--   CHECK (epic_id IS NULL OR project_id IS NOT NULL),     -- an epic implies a project
--   (the existing weekly/weekdays CHECK stays)
-- The store verifies on write that the epic belongs to the item's project.

ALTER TABLE sessions ADD COLUMN epic_id TEXT REFERENCES epics (id);
CREATE INDEX sessions_by_epic_day ON sessions (epic_id, day);
ALTER TABLE day_plan ADD COLUMN epic_id TEXT REFERENCES epics (id);

CREATE TABLE resources (
    id          TEXT PRIMARY KEY,
    item_id     TEXT REFERENCES items (id),
    epic_id     TEXT REFERENCES epics (id),
    kind        TEXT NOT NULL CHECK (kind IN ('note', 'link', 'reminder')),
    title       TEXT,
    body        TEXT,
    url         TEXT,
    remind_at   TEXT,
    done        INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0, 1)),
    position    INTEGER NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    version     INTEGER NOT NULL CHECK (version >= 1),
    CHECK ((item_id IS NULL) != (epic_id IS NULL)),          -- exactly one parent
    CHECK (kind != 'note' OR body IS NOT NULL),
    CHECK (kind != 'link' OR url IS NOT NULL),
    CHECK (kind != 'reminder' OR remind_at IS NOT NULL)
);
CREATE INDEX resources_by_item ON resources (item_id, position);
CREATE INDEX resources_by_epic ON resources (epic_id, position);
CREATE INDEX resources_due ON resources (kind, done, remind_at);
```

Rules:
- Backlog = `block IS NULL`. Backlog items never enter `day_plan`, snapshots, capacity or
  fulfilment. `item.schedule` sets `block`+`start_day`; `item.backlog` clears both (one-time only).
  A frozen past day that already lists the item keeps it.
- Session `epic_id` is copied from the item at write, or given directly for epic-only work
  (`session.log/start {epic_id}` then also sets `project_id` from the epic). It requires a
  `project_id` on the session (enforced in core; CHECK added only if it does not need a sessions
  rebuild).
- `epic.remove` is a soft delete allowed only with no sessions; otherwise finish it.
- Unassigned work is first-class in stats: `stats.table` rows with `key: "none"` ("No project",
  "No tag", "No task", and with 004 "No epic"); `sessions.list` rows carry `project`/`item` null;
  filters accept `"none"` (C5 also adds `"none"` to `stats.get`, which lacks it after phase 2).
- `changes` entity names: `epics`, `resources`; session notes are reported under `sessions`.

## 005 — phase 4 (worker C8)

```sql
CREATE TABLE goals (
    id          TEXT PRIMARY KEY,
    project_id  TEXT REFERENCES projects (id),
    epic_id     TEXT REFERENCES epics (id),
    item_id     TEXT REFERENCES items (id),
    period      TEXT NOT NULL CHECK (period IN ('day', 'week')),
    metric_id   TEXT REFERENCES metrics (id),          -- NULL: the goal is on minutes
    direction   TEXT NOT NULL DEFAULT 'at_least' CHECK (direction IN ('at_least', 'at_most')),
    target      REAL NOT NULL CHECK (target > 0),
    starts_on   TEXT NOT NULL,                          -- first day of its first period
    ends_on     TEXT,                                   -- last day of its last period; NULL = active
    replaces_id TEXT REFERENCES goals (id),             -- predecessor when a target was edited
    position    INTEGER NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    version     INTEGER NOT NULL CHECK (version >= 1),
    CHECK ((project_id IS NOT NULL) + (epic_id IS NOT NULL) + (item_id IS NOT NULL) <= 1),
    CHECK (metric_id IS NOT NULL OR direction = 'at_least'),
    CHECK (ends_on IS NULL OR ends_on >= starts_on)
);
CREATE INDEX goals_active ON goals (ends_on, deleted_at);

CREATE TABLE reviews (
    id          TEXT PRIMARY KEY,
    week        TEXT NOT NULL,                          -- the Monday, YYYY-MM-DD
    note        TEXT NOT NULL CHECK (length(note) BETWEEN 1 AND 5000),
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    version     INTEGER NOT NULL CHECK (version >= 1)
);
CREATE UNIQUE INDEX reviews_by_week ON reviews (week) WHERE deleted_at IS NULL;
CREATE INDEX sessions_by_day ON sessions (day);
```

Rules:
- Progress, history, insights and the review summary are computed from sessions (and
  `session_values`, `session_notes`) at read time; nothing derived is stored.
- A goal with no scope ids is on all work. A goal's metric must be one its scope asks (or any
  built-in for all-work goals); the store checks this on write.
- `goal.edit` never changes a stored target: it sets `ends_on` on the old row to the last day of
  the previous period (or soft-deletes it if its first period has not ended) and inserts the
  successor with `replaces_id`, in one write.
- Week periods start Monday; a `day` goal's period is the tracking day (Sleep boundary), the same
  `day` value sessions carry.
- Removing a project/epic/item ends its goals (`ends_on` = last full period), never deletes them.
- `changes` entity names: `goals`, `reviews`.

## Review gates

1. After C2's migration 003 checkpoint, C5's migration 004 checkpoint and C8's migration 005 checkpoint, an independent
   reviewer (not the author) checks the migration, constraints, indexes, layering and the
   migration tests against this file before any frontend builds on it.
2. The orchestrator migrates a copy of the live database and diffs row counts per table before
   installing.
