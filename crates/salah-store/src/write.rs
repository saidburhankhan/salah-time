//! The write transaction: entity upserts that bump the store revision once per
//! transaction and append one `changes` row per entity written.

use crate::{
    Store,
    codec::{self, storage_error},
};
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{OptionalExtension, params};
use salah_domain::{
    error::Error,
    item::PrayerMark,
    tracking::{
        DayBlock, DayClock, DayPlanLine, Item, Metric, Project, ProjectMetric, Session,
        SessionNote, Tag,
    },
};
use std::{
    cell::{Cell, RefCell},
    ops::Deref,
};

/// An open `BEGIN IMMEDIATE` transaction. Reads go through `Deref<Target = Store>`
/// and therefore see the transaction's own writes.
pub struct WriteTx<'a> {
    pub(crate) store: &'a Store,
    pub(crate) revision: Cell<Option<i64>>,
    pub(crate) op_id: RefCell<Option<String>>,
}

impl Deref for WriteTx<'_> {
    type Target = Store;

    fn deref(&self) -> &Store {
        self.store
    }
}

impl WriteTx<'_> {
    /// Tags every change written after this call with the client op id.
    pub fn set_op_id(&self, op_id: Option<&str>) {
        *self.op_id.borrow_mut() = op_id.map(str::to_owned);
    }

    /// Stores the method and result of `op_id` for idempotent replay, at the
    /// revision this transaction produced (or the current one if it wrote nothing).
    pub fn record_applied_op(
        &self,
        op_id: &str,
        method: &str,
        result_json: &str,
    ) -> Result<(), Error> {
        let revision = match self.revision.get() {
            Some(revision) => revision,
            None => self.store.revision()?,
        };
        self.execute(
            "INSERT INTO applied_ops (op_id, method, revision, result_json, at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT DO NOTHING",
            params![
                op_id,
                method,
                revision,
                result_json,
                codec::timestamp(Utc::now())
            ],
        )
    }

    pub fn put_project(&self, project: &Project) -> Result<i64, Error> {
        let open = self
            .one(
                "SELECT 1 FROM project_pauses WHERE project_id=?1 AND to_day IS NULL",
                [&project.id],
                |_| Ok(()),
            )?
            .is_some();
        if open != project.archived {
            return Err(Error::validation(
                "archived",
                "change the project and its pause span together",
            ));
        }
        let (version, op) = self.next_version("projects", &project.id, project.deleted_at)?;
        self.execute(
            "INSERT INTO projects
                 (id, name, color, note, archived, created_at, updated_at, deleted_at, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT (id) DO UPDATE SET
                 name = excluded.name,
                 color = excluded.color,
                 note = excluded.note,
                 archived = excluded.archived,
                 updated_at = excluded.updated_at,
                 deleted_at = excluded.deleted_at,
                 version = excluded.version",
            params![
                project.id,
                project.name,
                project.color,
                project.note,
                project.archived,
                codec::timestamp(project.created_at),
                codec::timestamp(project.updated_at),
                project.deleted_at.map(codec::timestamp),
                version,
            ],
        )?;
        self.execute(
            "DELETE FROM project_tags WHERE project_id = ?1",
            [&project.id],
        )?;
        for tag_id in &project.tag_ids {
            self.execute(
                "INSERT INTO project_tags (project_id, tag_id) VALUES (?1, ?2)",
                params![project.id, tag_id],
            )?;
        }
        self.log_change("project", &project.id, op, version)
    }

    /// Replaces the metrics a project asks for. Part of the project entity, so
    /// it is logged as a project change by the caller's `put_project`.
    pub fn set_project_metrics(
        &self,
        project_id: &str,
        rows: &[ProjectMetric],
    ) -> Result<(), Error> {
        self.execute(
            "DELETE FROM project_metrics WHERE project_id = ?1",
            [project_id],
        )?;
        for row in rows {
            self.execute(
                "INSERT INTO project_metrics (project_id, metric_id, required, position)
                 VALUES (?1, ?2, ?3, ?4)",
                params![row.project_id, row.metric_id, row.required, row.position],
            )?;
        }
        Ok(())
    }

    pub fn put_epic(&self, epic: &salah_domain::epic::Epic) -> Result<i64, Error> {
        let (version, op) = self.next_version("epics", &epic.id, epic.deleted_at)?;
        self.execute(
            "INSERT INTO epics
                (id, project_id, name, note, status, finished_at, position, created_at, updated_at, deleted_at, version)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, note=excluded.note,
                status=excluded.status, finished_at=excluded.finished_at, updated_at=excluded.updated_at,
                deleted_at=excluded.deleted_at, version=excluded.version",
            params![epic.id, epic.project_id, epic.name, epic.note, codec::name(&epic.status),
                    epic.finished_at.map(codec::timestamp), epic.position, codec::timestamp(epic.created_at),
                    codec::timestamp(epic.updated_at), epic.deleted_at.map(codec::timestamp), version],
        )?;
        self.log_change("epics", &epic.id, op, version)
    }

    pub fn put_tag(&self, tag: &Tag) -> Result<i64, Error> {
        let (version, op) = self.next_version("tags", &tag.id, tag.deleted_at)?;
        self.execute(
            "INSERT INTO tags (id, name, color, created_at, updated_at, deleted_at, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (id) DO UPDATE SET
                 name = excluded.name,
                 color = excluded.color,
                 updated_at = excluded.updated_at,
                 deleted_at = excluded.deleted_at,
                 version = excluded.version",
            params![
                tag.id,
                tag.name,
                tag.color,
                codec::timestamp(tag.created_at),
                codec::timestamp(tag.updated_at),
                tag.deleted_at.map(codec::timestamp),
                version,
            ],
        )?;
        self.log_change("tag", &tag.id, op, version)
    }

    pub fn put_metric(&self, metric: &Metric) -> Result<i64, Error> {
        let (version, op) = self.next_version("metrics", &metric.id, metric.deleted_at)?;
        self.execute(
            "INSERT INTO metrics
                 (id, name, unit, kind, scale_max, rollup, builtin, archived,
                  created_at, updated_at, deleted_at, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT (id) DO UPDATE SET
                 name = excluded.name,
                 unit = excluded.unit,
                 kind = excluded.kind,
                 scale_max = excluded.scale_max,
                 rollup = excluded.rollup,
                 archived = excluded.archived,
                 updated_at = excluded.updated_at,
                 deleted_at = excluded.deleted_at,
                 version = excluded.version",
            params![
                metric.id,
                metric.name,
                metric.unit,
                codec::name(&metric.kind),
                metric.scale_max,
                codec::name(&metric.rollup),
                metric.builtin,
                metric.archived,
                codec::timestamp(metric.created_at),
                codec::timestamp(metric.updated_at),
                metric.deleted_at.map(codec::timestamp),
                version,
            ],
        )?;
        self.log_change("metric", &metric.id, op, version)
    }

    /// Stores the settings needed to reconstruct unread past days.
    pub fn record_config_change(&self, today: NaiveDate, previous: &str) -> Result<(), Error> {
        self.execute(
            "INSERT INTO config_history (until_day, config_json) VALUES (?1, ?2)
            ON CONFLICT (until_day) DO NOTHING",
            params![codec::day(today), previous],
        )?;
        let revision = self.revision_for_write()?;
        self.log_change("config", "config", "update", revision)?;
        Ok(())
    }

    pub fn preserve_item(&self, item: &Item, today: NaiveDate) -> Result<(), Error> {
        let body = serde_json::to_string(item).map_err(|e| Error::Internal(e.to_string()))?;
        self.execute(
            "INSERT INTO item_history (item_id, until_day, item_json) VALUES (?1, ?2, ?3)
             ON CONFLICT (item_id, until_day) DO NOTHING",
            params![item.id, codec::day(today), body],
        )
    }

    pub fn put_item(&self, item: &Item) -> Result<i64, Error> {
        if let Some(id) = &item.epic_id {
            let epic = self
                .epic(id)?
                .filter(|e| e.deleted_at.is_none())
                .ok_or_else(|| Error::NotFound {
                    what: "epic",
                    id: id.clone(),
                })?;
            if Some(&epic.project_id) != item.project_id.as_ref() {
                return Err(Error::validation(
                    "epic_id",
                    "the epic must belong to the item's project",
                ));
            }
        }
        let (version, op) = self.next_version("items", &item.id, item.deleted_at)?;
        self.execute(
            "INSERT INTO items
                 (id, title, project_id, block, repeat, weekdays, start_day, planned_minutes,
                  position, created_at, updated_at, deleted_at, version, metrics_custom, epic_id, end_day)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
             ON CONFLICT (id) DO UPDATE SET
                 title = excluded.title,
                 project_id = excluded.project_id,
                 block = excluded.block,
                 repeat = excluded.repeat,
                 weekdays = excluded.weekdays,
                 start_day = excluded.start_day,
                 planned_minutes = excluded.planned_minutes,
                 position = excluded.position,
                 updated_at = excluded.updated_at,
                 deleted_at = excluded.deleted_at,
                 version = excluded.version,
                 metrics_custom = excluded.metrics_custom,
                 epic_id = excluded.epic_id,
                 end_day = excluded.end_day",
            params![
                item.id,
                item.title,
                item.project_id,
                item.block.as_ref().map(codec::name),
                codec::name(&item.repeat),
                item.weekdays.map(|days| days.0),
                item.day.map(codec::day),
                item.planned_minutes,
                item.position,
                codec::timestamp(item.created_at),
                codec::timestamp(item.updated_at),
                item.deleted_at.map(codec::timestamp),
                version,
                item.metric_ids.is_some(),
                item.epic_id,
                item.end_day.map(codec::day),
            ],
        )?;
        self.execute("DELETE FROM item_metrics WHERE item_id = ?1", [&item.id])?;
        if let Some(ids) = &item.metric_ids {
            for (position, metric_id) in ids.iter().enumerate() {
                self.execute(
                    "INSERT INTO item_metrics (item_id, metric_id, position) VALUES (?1, ?2, ?3)",
                    params![item.id, metric_id, position as i64],
                )?;
            }
        }
        self.log_change("item", &item.id, op, version)
    }

    pub fn put_session(&self, session: &Session) -> Result<i64, Error> {
        let (version, op) = self.next_version("sessions", &session.id, session.deleted_at)?;
        self.execute(
            "INSERT INTO sessions
                 (id, item_id, project_id, day, block, started_at, ended_at, minutes, note,
                  created_at, updated_at, deleted_at, version, epic_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT (id) DO UPDATE SET
                 day = excluded.day,
                 block = excluded.block,
                 started_at = excluded.started_at,
                 ended_at = excluded.ended_at,
                 minutes = excluded.minutes,
                 note = excluded.note,
                 updated_at = excluded.updated_at,
                 deleted_at = excluded.deleted_at,
                 version = excluded.version",
            params![
                session.id,
                session.item_id,
                session.project_id,
                codec::day(session.block.day),
                codec::name(&session.block.start),
                session.started_at.map(codec::timestamp),
                session.ended_at.map(codec::timestamp),
                session.minutes,
                session.note,
                codec::timestamp(session.created_at),
                codec::timestamp(session.updated_at),
                session.deleted_at.map(codec::timestamp),
                version,
                session.epic_id,
            ],
        )?;
        self.execute(
            "DELETE FROM session_values WHERE session_id = ?1",
            [&session.id],
        )?;
        for value in &session.values {
            self.execute(
                "INSERT INTO session_values (session_id, metric_id, value) VALUES (?1, ?2, ?3)",
                params![session.id, value.metric_id, value.value],
            )?;
        }
        self.log_change("session", &session.id, op, version)
    }

    /// Replace a single metric value while leaving the other samples in place.
    pub fn put_session_value(
        &self,
        session_id: &str,
        metric_id: &str,
        value: f64,
        now: DateTime<Utc>,
    ) -> Result<i64, Error> {
        self.execute(
            "INSERT INTO session_values (session_id, metric_id, value) VALUES (?1, ?2, ?3)
             ON CONFLICT (session_id, metric_id) DO UPDATE SET value = excluded.value",
            params![session_id, metric_id, value],
        )?;
        self.touch_session(session_id, now)
    }

    pub fn add_session_note(&self, session_id: &str, note: &SessionNote) -> Result<i64, Error> {
        self.execute(
            "INSERT INTO session_notes (id, session_id, at, text, created_at, updated_at, version)
             VALUES (?1, ?2, ?3, ?4, ?3, ?3, 1)",
            params![note.id, session_id, codec::timestamp(note.at), note.text],
        )?;
        self.touch_session(session_id, note.at)
    }

    pub fn remove_session_notes(
        &self,
        session_id: &str,
        note_ids: &[String],
        now: DateTime<Utc>,
    ) -> Result<(), Error> {
        for id in note_ids {
            let deleted = self.store.one(
                "UPDATE session_notes SET deleted_at = ?3, updated_at = ?3, version = version + 1
                 WHERE session_id = ?1 AND id = ?2 AND deleted_at IS NULL RETURNING id",
                params![session_id, id, codec::timestamp(now)],
                |row| row.get::<_, String>(0),
            )?;
            if deleted.is_none() {
                return Err(Error::validation(
                    "remove_note_ids",
                    format!("no live note {id:?} on this session"),
                ));
            }
        }
        Ok(())
    }

    pub fn remove_all_session_notes(
        &self,
        session_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), Error> {
        self.execute(
            "UPDATE session_notes SET deleted_at = ?2, updated_at = ?2, version = version + 1
             WHERE session_id = ?1 AND deleted_at IS NULL",
            params![session_id, codec::timestamp(now)],
        )
    }

    fn touch_session(&self, session_id: &str, now: DateTime<Utc>) -> Result<i64, Error> {
        let version = self
            .store
            .conn
            .query_row(
                "UPDATE sessions SET updated_at = ?2, version = version + 1
             WHERE id = ?1 AND deleted_at IS NULL RETURNING version",
                params![session_id, codec::timestamp(now)],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        self.log_change("session", session_id, "update", version)
    }

    pub fn put_mark(&self, mark: &PrayerMark) -> Result<i64, Error> {
        let (version, op) = self.next_version("prayer_marks", &mark.id, mark.deleted_at)?;
        self.execute(
            "INSERT INTO prayer_marks
                 (id, day, prayer, marked_at, created_at, updated_at, deleted_at, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT (id) DO UPDATE SET
                 marked_at = excluded.marked_at,
                 updated_at = excluded.updated_at,
                 deleted_at = excluded.deleted_at,
                 version = excluded.version",
            params![
                mark.id,
                codec::day(mark.day),
                codec::name(&mark.prayer),
                codec::timestamp(mark.marked_at),
                codec::timestamp(mark.created_at),
                codec::timestamp(mark.updated_at),
                mark.deleted_at.map(codec::timestamp),
                version,
            ],
        )?;
        self.log_change("prayer_mark", &mark.id, op, version)
    }

    /// Writes the frozen snapshot of a past day once: its blocks, its plan,
    /// and the marker that it was written. Snapshots are derived data, so they
    /// neither bump the revision nor appear in the change log.
    pub fn write_day(
        &self,
        day: NaiveDate,
        clock: &DayClock,
        blocks: &[DayBlock],
        plan: &[DayPlanLine],
    ) -> Result<(), Error> {
        for block in blocks {
            self.execute(
                "INSERT INTO day_blocks (day, block, starts_at, ends_at, source)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    codec::day(block.day),
                    codec::name(&block.block),
                    codec::timestamp(block.starts_at),
                    codec::timestamp(block.ends_at),
                    codec::name(&block.source),
                ],
            )?;
        }
        for line in plan {
            self.execute(
                "INSERT INTO day_plan (day, block, item_id, project_id, planned_minutes, epic_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    codec::day(line.day),
                    codec::name(&line.block),
                    line.item_id,
                    line.project_id,
                    line.planned_minutes,
                    line.epic_id,
                ],
            )?;
        }
        self.execute(
            "INSERT INTO days_written (day, written_at) VALUES (?1, ?2)",
            params![codec::day(day), codec::timestamp(Utc::now())],
        )?;
        self.put_day_clock(day, clock)
    }

    pub fn put_day_clock(&self, day: NaiveDate, clock: &DayClock) -> Result<(), Error> {
        self.execute(
            "INSERT INTO day_clocks (day, offset_seconds, sunrise_minutes, sleep_at) VALUES (?1, ?2, ?3, ?4)",
            params![codec::day(day), clock.offset_seconds, clock.sunrise_minutes, codec::timestamp(clock.sleep_at)],
        )
    }

    pub(crate) fn execute(&self, sql: &str, params: impl rusqlite::Params) -> Result<(), Error> {
        self.store
            .conn
            .prepare_cached(sql)
            .and_then(|mut statement| statement.execute(params))
            .map(drop)
            .map_err(storage_error)
    }

    /// The version an upsert writes and the change operation it represents.
    pub(crate) fn next_version(
        &self,
        table: &str,
        id: &str,
        deleted_at: Option<DateTime<Utc>>,
    ) -> Result<(i64, &'static str), Error> {
        let stored: Option<(i64, Option<String>)> = self
            .store
            .conn
            .query_row(
                &format!("SELECT version, deleted_at FROM {table} WHERE id = ?1"),
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(storage_error)?;
        Ok(match stored {
            None => (1, "insert"),
            Some((version, None)) if deleted_at.is_some() => (version + 1, "delete"),
            Some((version, _)) => (version + 1, "update"),
        })
    }

    /// Bumps the revision on the first write of the transaction.
    fn revision_for_write(&self) -> Result<i64, Error> {
        if let Some(revision) = self.revision.get() {
            return Ok(revision);
        }
        let revision = self
            .store
            .conn
            .query_row(
                "UPDATE meta SET revision = revision + 1 RETURNING revision",
                [],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        self.revision.set(Some(revision));
        Ok(revision)
    }

    pub(crate) fn log_change(
        &self,
        entity: &str,
        id: &str,
        op: &str,
        version: i64,
    ) -> Result<i64, Error> {
        let revision = self.revision_for_write()?;
        self.execute(
            "INSERT INTO changes (revision, entity, entity_id, op, version, at, op_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                revision,
                entity,
                id,
                op,
                version,
                codec::timestamp(Utc::now()),
                self.op_id.borrow().as_deref(),
            ],
        )?;
        Ok(version)
    }
}
