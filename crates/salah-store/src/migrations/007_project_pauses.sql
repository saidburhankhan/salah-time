CREATE TABLE project_pauses (
    project_id TEXT NOT NULL REFERENCES projects(id),
    from_day TEXT NOT NULL,
    to_day TEXT,
    PRIMARY KEY (project_id, from_day),
    CHECK (to_day IS NULL OR to_day >= from_day)
);
CREATE UNIQUE INDEX project_pauses_open ON project_pauses(project_id) WHERE to_day IS NULL;
-- The runner seeds archived projects in this transaction using the configured tracking day.
