ALTER TABLE items ADD COLUMN end_day TEXT
    CHECK (end_day IS NULL OR (
        repeat IN ('daily', 'weekly') AND start_day IS NOT NULL AND end_day >= start_day
        AND length(end_day) = 10 AND date(end_day, '+0 days') IS end_day
    ));
