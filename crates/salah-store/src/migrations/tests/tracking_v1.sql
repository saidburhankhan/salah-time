-- A week of tracking data, including archived work and linked tombstones.
INSERT INTO projects VALUES
    ('p', 'Reading', '#123456', 'Library', 0, '2031-05-10T09:00:00Z', '2031-05-13T17:00:00Z', NULL, 3),
    ('a', 'Last term', '#999999', NULL, 1, '2031-05-10T09:00:00Z', '2031-05-13T17:00:00Z', NULL, 2),
    ('d', 'Removed', '#888888', NULL, 0, '2031-05-10T09:00:00Z', '2031-05-13T17:00:00Z', '2031-05-13T17:00:00Z', 2);
INSERT INTO tags VALUES
    ('t', 'Study', '#333333', '2031-05-10T09:00:00Z', '2031-05-11T13:00:00Z', NULL, 2);
INSERT INTO project_tags VALUES ('p', 't'), ('a', 't');
INSERT INTO metrics VALUES
    ('pages', 'Pages', 'pp', 'count', NULL, 'sum', 0, 0, '2031-05-10T09:00:00Z', '2031-05-10T09:00:00Z', NULL, 1);
INSERT INTO project_metrics VALUES ('p', 'focus', 1, 2), ('p', 'pages', 0, 7), ('d', 'quality', 1, 0);

INSERT INTO items VALUES
    ('once', 'Return a book', 'p', 'asr', 'none', NULL, '2031-05-12', NULL, 0, '2031-05-10T09:00:00Z', '2031-05-10T09:00:00Z', NULL, 1),
    ('daily', 'Read', 'p', 'fajr', 'daily', NULL, '2031-05-10', 30, 1, '2031-05-10T09:00:00Z', '2031-05-13T17:00:00Z', NULL, 2),
    ('weekly', 'Review', NULL, 'isha', 'weekly', 5, '2031-05-10', 15, 0, '2031-05-10T09:00:00Z', '2031-05-10T09:00:00Z', NULL, 1);
INSERT INTO items VALUES
    ('removed', 'Retired task', 'd', 'asr', 'none', NULL, '2031-05-11', 15, 0, '2031-05-10T09:00:00Z', '2031-05-13T17:00:00Z', '2031-05-13T17:00:00Z', 3);
INSERT INTO sessions VALUES
    ('run', 'weekly', NULL, '2031-05-14', 'isha', '2031-05-14T17:00:00Z', NULL, NULL, NULL, '2031-05-14T17:00:00Z', '2031-05-14T17:00:00Z', NULL, 1),
    ('work', 'daily', 'p', '2031-05-12', 'fajr', '2031-05-12T03:00:00Z', '2031-05-12T03:27:00Z', 27, 'Chapter 2', '2031-05-12T03:00:00Z', '2031-05-12T03:27:00Z', NULL, 2),
    ('tick', 'once', 'p', '2031-05-12', 'asr', NULL, NULL, NULL, 'Returned', '2031-05-12T12:00:00Z', '2031-05-12T12:00:00Z', NULL, 1);
INSERT INTO sessions VALUES
    ('removed-work', 'removed', 'd', '2031-05-11', 'asr', NULL, NULL, 15, 'Old note', '2031-05-11T12:00:00Z', '2031-05-13T17:00:00Z', '2031-05-13T17:00:00Z', 2);
INSERT INTO session_values VALUES ('removed-work', 'pages', 3);
INSERT INTO session_values VALUES ('work', 'focus', 4), ('work', 'pages', 12), ('tick', 'quality', 5);

INSERT INTO days_written VALUES ('2031-05-12', '2031-05-13T17:00:00Z');
INSERT INTO day_blocks VALUES
    ('2031-05-12', 'fajr', '2031-05-12T00:00:00Z', '2031-05-12T07:00:00Z', 'calculated');
INSERT INTO day_plan VALUES ('2031-05-12', 'fajr', 'daily', 'p', 30);
INSERT INTO prayer_marks VALUES
    ('m', '2031-05-12', 'fajr', '2031-05-12T01:00:00Z', '2031-05-12T01:00:00Z', '2031-05-12T01:00:00Z', NULL, 1);
UPDATE meta SET revision = 18;
INSERT INTO changes (revision, entity, entity_id, op, version, at, op_id)
    VALUES (18, 'session', 'work', 'update', 2, '2031-05-12T03:27:00Z', 'stop');
INSERT INTO applied_ops VALUES
    ('stop', 'session.stop', 18, '{"revision":18,"changed":{"sessions":[{"id":"work","minutes":27}]}}', '2031-05-12T03:27:00Z');
