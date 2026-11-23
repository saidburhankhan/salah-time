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
