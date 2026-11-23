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

CREATE TABLE items_new (
    id               TEXT PRIMARY KEY,
    title            TEXT NOT NULL,
    project_id       TEXT REFERENCES projects (id),
    block            TEXT CHECK (block IS NULL OR block IN ('night', 'fajr', 'dhuhr', 'asr', 'maghrib', 'isha')),
    repeat           TEXT NOT NULL CHECK (repeat IN ('none', 'daily', 'weekly')),
    weekdays         INTEGER,
    start_day        TEXT,
    planned_minutes  INTEGER CHECK (planned_minutes IS NULL OR planned_minutes >= 0),
    position         INTEGER NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    deleted_at       TEXT,
    version          INTEGER NOT NULL CHECK (version >= 1),
    metrics_custom   INTEGER NOT NULL DEFAULT 0 CHECK (metrics_custom IN (0, 1)),
    epic_id          TEXT REFERENCES epics (id),
    CHECK ((repeat = 'weekly' AND weekdays BETWEEN 1 AND 127)
        OR (repeat != 'weekly' AND weekdays IS NULL)),
    CHECK ((block IS NULL) = (start_day IS NULL)),
    CHECK (block IS NOT NULL OR repeat = 'none'),
    CHECK (epic_id IS NULL OR project_id IS NOT NULL)
);
INSERT INTO items_new (rowid, id, title, project_id, block, repeat, weekdays, start_day,
                       planned_minutes, position, created_at, updated_at, deleted_at, version, metrics_custom)
SELECT rowid, id, title, project_id, block, repeat, weekdays, start_day,
       planned_minutes, position, created_at, updated_at, deleted_at, version, metrics_custom
FROM items;
DROP TABLE items;
ALTER TABLE items_new RENAME TO items;
CREATE INDEX items_by_project ON items (project_id);

ALTER TABLE sessions ADD COLUMN epic_id TEXT REFERENCES epics (id)
    CHECK (epic_id IS NULL OR project_id IS NOT NULL);
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
    CHECK ((item_id IS NULL) != (epic_id IS NULL)),
    CHECK (kind != 'note' OR body IS NOT NULL),
    CHECK (kind != 'link' OR url IS NOT NULL),
    CHECK (kind != 'reminder' OR remind_at IS NOT NULL)
);
CREATE INDEX resources_by_item ON resources (item_id, position);
CREATE INDEX resources_by_epic ON resources (epic_id, position);
CREATE INDEX resources_due ON resources (kind, done, remind_at);
