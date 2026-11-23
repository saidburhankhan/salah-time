-- Phase 3 data on top of the v1 week (tracking_v1.sql) and the v3 rows of phase3.rs:
-- a second tagged project with epics, an untagged project, a backlog task, a removed task
-- that still has work, epic-only work, bare project work in the archived project, a
-- task without a project, a removed session, resources, and a link to a removed tag.
INSERT INTO projects (id, name, color, note, archived, created_at, updated_at, deleted_at, version) VALUES
    ('q', 'Career', '#445566', NULL, 0, '2031-05-11T08:00:00Z', '2031-05-11T08:00:00Z', NULL, 1),
    ('n', 'Errands', '#556677', NULL, 0, '2031-05-11T08:00:00Z', '2031-05-11T08:00:00Z', NULL, 1);
INSERT INTO tags (id, name, color, created_at, updated_at, deleted_at, version) VALUES
    ('u', 'Quran', '#444444', '2031-05-11T08:00:00Z', '2031-05-11T08:00:00Z', NULL, 1),
    ('x', 'Old', '#555555', '2031-05-10T09:00:00Z', '2031-05-13T17:00:00Z', '2031-05-13T17:00:00Z', 2);
-- Reads ignore the removed tag 'x'; 005 must not copy this link onto tasks.
INSERT INTO project_tags (project_id, tag_id) VALUES ('q', 't'), ('q', 'u'), ('q', 'x');
INSERT INTO project_metrics (project_id, metric_id, required, position) VALUES ('q', 'focus', 0, 0);

INSERT INTO epics (id, project_id, name, note, status, finished_at, position,
                   created_at, updated_at, deleted_at, version) VALUES
    ('e', 'q', 'DBMS course', 'Lectures 1-12', 'active', NULL, 0,
     '2031-05-11T08:00:00Z', '2031-05-11T08:00:00Z', NULL, 1),
    ('f', 'q', 'CV refresh', NULL, 'finished', '2031-05-13T10:00:00Z', 1,
     '2031-05-11T08:00:00Z', '2031-05-13T10:00:00Z', NULL, 2);

INSERT INTO items (id, title, project_id, block, repeat, weekdays, start_day, planned_minutes,
                   position, created_at, updated_at, deleted_at, version, metrics_custom, epic_id) VALUES
    ('course', 'Lecture', 'q', 'dhuhr', 'weekly', 31, '2031-05-11', 60, 0,
     '2031-05-11T08:00:00Z', '2031-05-11T08:00:00Z', NULL, 1, 1, 'e'),
    ('backlog', 'Read chapter 3', 'q', NULL, 'none', NULL, NULL, 45, 1,
     '2031-05-11T08:00:00Z', '2031-05-12T09:00:00Z', NULL, 2, 0, 'e'),
    ('dropped', 'Cover letter', 'q', 'maghrib', 'none', NULL, '2031-05-12', 30, 2,
     '2031-05-11T08:00:00Z', '2031-05-13T10:00:00Z', '2031-05-13T10:00:00Z', 2, 0, 'f'),
    ('errand', 'Groceries', 'n', 'asr', 'none', NULL, '2031-05-13', NULL, 0,
     '2031-05-11T08:00:00Z', '2031-05-11T08:00:00Z', NULL, 1, 0, NULL);
INSERT INTO item_metrics (item_id, metric_id, position) VALUES ('course', 'focus', 0);

INSERT INTO sessions (id, item_id, project_id, day, block, started_at, ended_at, minutes, note,
                      created_at, updated_at, deleted_at, version, epic_id) VALUES
    ('lecture', 'course', 'q', '2031-05-12', 'dhuhr', '2031-05-12T08:00:00Z', '2031-05-12T08:50:00Z',
     50, NULL, '2031-05-12T08:00:00Z', '2031-05-12T08:50:00Z', NULL, 2, 'e'),
    ('lecture-tick', 'course', 'q', '2031-05-13', 'dhuhr', NULL, NULL, NULL, 'Attended',
     '2031-05-13T08:00:00Z', '2031-05-13T08:00:00Z', NULL, 1, 'e'),
    ('discarded', 'course', 'q', '2031-05-13', 'dhuhr', '2031-05-13T09:00:00Z', '2031-05-13T09:05:00Z',
     5, NULL, '2031-05-13T09:00:00Z', '2031-05-13T09:06:00Z', '2031-05-13T09:06:00Z', 2, 'e'),
    ('cv', 'dropped', 'q', '2031-05-12', 'maghrib', NULL, NULL, 40, 'Draft',
     '2031-05-12T14:00:00Z', '2031-05-12T14:00:00Z', NULL, 1, 'f'),
    ('cv-epic', NULL, 'q', '2031-05-13', 'isha', '2031-05-13T15:00:00Z', '2031-05-13T15:25:00Z',
     25, NULL, '2031-05-13T15:00:00Z', '2031-05-13T15:25:00Z', NULL, 2, 'f'),
    ('shop', 'errand', 'n', '2031-05-13', 'asr', '2031-05-13T11:00:00Z', '2031-05-13T11:20:00Z',
     20, NULL, '2031-05-13T11:00:00Z', '2031-05-13T11:20:00Z', NULL, 2, NULL),
    ('archive', NULL, 'a', '2031-05-11', 'fajr', NULL, NULL, 10, 'Old notes',
     '2031-05-11T02:00:00Z', '2031-05-11T02:00:00Z', NULL, 1, NULL),
    ('loose', 'weekly', NULL, '2031-05-13', 'isha', NULL, NULL, 15, NULL,
     '2031-05-13T16:00:00Z', '2031-05-13T16:00:00Z', NULL, 1, NULL);
INSERT INTO session_values (session_id, metric_id, value) VALUES
    ('lecture', 'focus', 4), ('lecture', 'distractions', 2), ('cv', 'quality', 3),
    ('cv-epic', 'focus', 5), ('shop', 'focus', 2), ('discarded', 'focus', 1);
INSERT INTO session_notes (id, session_id, at, text, created_at, updated_at, deleted_at, version) VALUES
    ('n2', 'lecture', '2031-05-12T08:20:00Z', 'Normal forms',
     '2031-05-12T08:20:00Z', '2031-05-12T08:20:00Z', NULL, 1);

INSERT INTO day_plan (day, block, item_id, project_id, planned_minutes, epic_id) VALUES
    ('2031-05-12', 'dhuhr', 'course', 'q', 60, 'e');
INSERT INTO resources (id, item_id, epic_id, kind, title, body, url, remind_at, done, position,
                       created_at, updated_at, deleted_at, version) VALUES
    ('r-note', 'course', NULL, 'note', 'Room', 'B-204', NULL, NULL, 0, 0,
     '2031-05-11T08:00:00Z', '2031-05-11T08:00:00Z', NULL, 1),
    ('r-link', NULL, 'e', 'link', 'Syllabus', NULL, 'https://example.org/dbms', NULL, 0, 0,
     '2031-05-11T08:00:00Z', '2031-05-11T08:00:00Z', NULL, 1),
    ('r-remind', 'backlog', NULL, 'reminder', NULL, NULL, NULL, '2031-05-15T05:00:00Z', 0, 0,
     '2031-05-12T09:00:00Z', '2031-05-12T09:00:00Z', NULL, 1),
    ('r-done', 'errand', NULL, 'reminder', 'List', NULL, NULL, '2031-05-13T10:00:00Z', 1, 1,
     '2031-05-11T08:00:00Z', '2031-05-13T10:30:00Z', NULL, 2);

UPDATE meta SET revision = 19;
INSERT INTO changes (revision, entity, entity_id, op, version, at, op_id)
    VALUES (19, 'epics', 'f', 'update', 2, '2031-05-13T10:00:00Z', 'finish');
INSERT INTO applied_ops VALUES
    ('finish', 'epic.finish', 19, '{"revision":19}', '2031-05-13T10:00:00Z');
