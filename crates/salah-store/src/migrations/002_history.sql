CREATE TABLE IF NOT EXISTS day_clocks (
    day TEXT PRIMARY KEY REFERENCES days_written (day),
    offset_seconds INTEGER NOT NULL,
    sunrise_minutes INTEGER,
    sleep_at TEXT NOT NULL
);

-- Before the first edit of an item on each planning day. This also preserves
-- the original item for unread days older than the eager snapshot window.
CREATE TABLE IF NOT EXISTS item_history (
    item_id TEXT NOT NULL REFERENCES items (id),
    until_day TEXT NOT NULL,
    item_json TEXT NOT NULL,
    PRIMARY KEY (item_id, until_day)
);

-- Settings in effect before the first config change on each planning day.
CREATE TABLE IF NOT EXISTS config_history (
    until_day TEXT PRIMARY KEY,
    config_json TEXT NOT NULL
);
