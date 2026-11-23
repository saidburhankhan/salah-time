-- Phase 4: goals, weekly reviews and tags on tasks (data-model.md "005 — phase 4").
-- Purely additive: three new tables and their indexes; no existing row changes.

-- A target for a day or a week on minutes (metric_id NULL) or on one metric, for all
-- work (no scope id) or for one project, epic or task. An edited target ends the old row
-- and inserts a successor pointing back at it through replaces_id.
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

-- One note per week; a removed review frees its week for a new one.
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

-- Tags on tasks (plan.md "Addendum: tags on tasks"); project_tags stays as each
-- project's default tags for new tasks. Every existing task, removed ones included,
-- starts with exactly its project's live tags, so statistics by tag keep their values.
CREATE TABLE item_tags (
    item_id TEXT NOT NULL REFERENCES items (id),
    tag_id  TEXT NOT NULL REFERENCES tags (id),
    PRIMARY KEY (item_id, tag_id)
);
CREATE INDEX item_tags_by_tag ON item_tags (tag_id);
INSERT INTO item_tags (item_id, tag_id)
    SELECT i.id, pt.tag_id FROM items i
    JOIN project_tags pt ON pt.project_id = i.project_id
    JOIN tags t ON t.id = pt.tag_id AND t.deleted_at IS NULL;
