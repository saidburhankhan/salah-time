-- Salah tracking schema, version 1.
--
-- Timestamps are RFC 3339 UTC text with millisecond precision; days are
-- `YYYY-MM-DD`. Every synced entity carries `created_at`, `updated_at`,
-- `deleted_at` (soft delete) and a `version` that grows with each write.

CREATE TABLE projects (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    color       TEXT NOT NULL,
    note        TEXT,
    archived    INTEGER NOT NULL CHECK (archived IN (0, 1)),
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    version     INTEGER NOT NULL CHECK (version >= 1)
);

CREATE TABLE tags (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    color       TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    version     INTEGER NOT NULL CHECK (version >= 1)
);

CREATE TABLE project_tags (
    project_id  TEXT NOT NULL REFERENCES projects (id),
    tag_id      TEXT NOT NULL REFERENCES tags (id),
    PRIMARY KEY (project_id, tag_id)
);

CREATE TABLE metrics (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    unit        TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('count', 'scale', 'check')),
    scale_max   INTEGER,
    rollup      TEXT NOT NULL CHECK (rollup IN ('sum', 'avg')),
    builtin     INTEGER NOT NULL CHECK (builtin IN (0, 1)),
    archived    INTEGER NOT NULL CHECK (archived IN (0, 1)),
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    version     INTEGER NOT NULL CHECK (version >= 1),
    CHECK ((kind = 'scale' AND scale_max >= 1) OR (kind != 'scale' AND scale_max IS NULL))
);

CREATE TABLE project_metrics (
    project_id  TEXT NOT NULL REFERENCES projects (id),
    metric_id   TEXT NOT NULL REFERENCES metrics (id),
    required    INTEGER NOT NULL CHECK (required IN (0, 1)),
    position    INTEGER NOT NULL,
    PRIMARY KEY (project_id, metric_id)
);

CREATE TABLE items (
    id               TEXT PRIMARY KEY,
    title            TEXT NOT NULL,
    project_id       TEXT REFERENCES projects (id),
    block            TEXT NOT NULL
                     CHECK (block IN ('night', 'fajr', 'dhuhr', 'asr', 'maghrib', 'isha')),
    repeat           TEXT NOT NULL CHECK (repeat IN ('none', 'daily', 'weekly')),
    weekdays         INTEGER,
    start_day        TEXT NOT NULL,
    planned_minutes  INTEGER CHECK (planned_minutes IS NULL OR planned_minutes >= 0),
    position         INTEGER NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    deleted_at       TEXT,
    version          INTEGER NOT NULL CHECK (version >= 1),
    CHECK ((repeat = 'weekly' AND weekdays BETWEEN 1 AND 127)
        OR (repeat != 'weekly' AND weekdays IS NULL))
);

CREATE TABLE sessions (
    id          TEXT PRIMARY KEY,
    item_id     TEXT REFERENCES items (id),
    project_id  TEXT REFERENCES projects (id),
    day         TEXT NOT NULL,
    block       TEXT NOT NULL
                CHECK (block IN ('night', 'fajr', 'dhuhr', 'asr', 'maghrib', 'isha', 'sleep')),
    started_at  TEXT,
    ended_at    TEXT,
    minutes     INTEGER CHECK (minutes IS NULL OR minutes >= 1),
    note        TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    version     INTEGER NOT NULL CHECK (version >= 1),
    CHECK (item_id IS NOT NULL OR project_id IS NOT NULL)
);

CREATE TABLE session_values (
    session_id  TEXT NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    metric_id   TEXT NOT NULL REFERENCES metrics (id),
    value       REAL NOT NULL,
    PRIMARY KEY (session_id, metric_id)
);

-- Snapshot of a day's prayer blocks, written the first time the day is used.
CREATE TABLE day_blocks (
    day        TEXT NOT NULL,
    block      TEXT NOT NULL
               CHECK (block IN ('night', 'fajr', 'dhuhr', 'asr', 'maghrib', 'isha', 'sleep')),
    starts_at  TEXT NOT NULL,
    ends_at    TEXT NOT NULL,
    source     TEXT NOT NULL CHECK (source IN ('official', 'calculated')),
    PRIMARY KEY (day, block)
);

-- Snapshot of the items planned on a day; past days never change.
CREATE TABLE day_plan (
    day              TEXT NOT NULL,
    block            TEXT NOT NULL
                     CHECK (block IN ('night', 'fajr', 'dhuhr', 'asr', 'maghrib', 'isha', 'sleep')),
    item_id          TEXT NOT NULL REFERENCES items (id),
    planned_minutes  INTEGER CHECK (planned_minutes IS NULL OR planned_minutes >= 0),
    PRIMARY KEY (day, block, item_id)
);

CREATE TABLE prayer_marks (
    id          TEXT PRIMARY KEY,
    day         TEXT NOT NULL,
    prayer      TEXT NOT NULL,
    marked_at   TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    version     INTEGER NOT NULL CHECK (version >= 1)
);

-- One row: the store-wide revision, bumped once per write transaction.
CREATE TABLE meta (
    id        INTEGER PRIMARY KEY CHECK (id = 1),
    revision  INTEGER NOT NULL
);
INSERT INTO meta (id, revision) VALUES (1, 0);

-- Append-only change log: one row per entity written.
CREATE TABLE changes (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    revision   INTEGER NOT NULL,
    entity     TEXT NOT NULL,
    entity_id  TEXT NOT NULL,
    op         TEXT NOT NULL CHECK (op IN ('insert', 'update', 'delete')),
    version    INTEGER NOT NULL,
    at         TEXT NOT NULL,
    op_id      TEXT
);

-- Idempotency: the stored result of every client op id.
CREATE TABLE applied_ops (
    op_id        TEXT PRIMARY KEY,
    revision     INTEGER NOT NULL,
    result_json  TEXT NOT NULL,
    at           TEXT NOT NULL
);

CREATE INDEX sessions_by_day ON sessions (day);
CREATE INDEX sessions_by_project_day ON sessions (project_id, day);
CREATE INDEX sessions_by_item ON sessions (item_id);
CREATE INDEX session_values_by_metric ON session_values (metric_id);
CREATE INDEX items_by_project ON items (project_id);
CREATE INDEX prayer_marks_by_day ON prayer_marks (day);
CREATE INDEX changes_by_revision ON changes (revision);
CREATE INDEX changes_by_entity ON changes (entity, entity_id);

-- Built-in metrics have fixed ids and timestamps, identical on every device.
INSERT INTO metrics
    (id, name, unit, kind, scale_max, rollup, builtin, archived,
     created_at, updated_at, deleted_at, version)
VALUES
    ('focus', 'Focus', '', 'scale', 5, 'avg', 1, 0,
     '1970-01-01T00:00:00.000Z', '1970-01-01T00:00:00.000Z', NULL, 1),
    ('quality', 'Quality', '', 'scale', 5, 'avg', 1, 0,
     '1970-01-01T00:00:00.000Z', '1970-01-01T00:00:00.000Z', NULL, 1);
