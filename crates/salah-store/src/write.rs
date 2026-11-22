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
    tracking::{DayBlock, DayPlanLine, Item, Metric, Project, ProjectMetric, Session, Tag},
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

    /// Stores the result of `op_id` for idempotent replay, at the revision
    /// this transaction produced (or the current one if it wrote nothing).
    pub fn record_applied_op(&self, op_id: &str, result_json: &str) -> Result<(), Error> {
        let revision = match self.revision.get() {
            Some(revision) => revision,
            None => self.store.revision()?,
        };
        self.execute(
            "INSERT INTO applied_ops (op_id, revision, result_json, at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT DO NOTHING",
            params![op_id, revision, result_json, codec::timestamp(Utc::now())],
        )
    }

    pub fn put_project(&self, project: &Project) -> Result<i64, Error> {
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

    pub fn put_item(&self, item: &Item) -> Result<i64, Error> {
        let (version, op) = self.next_version("items", &item.id, item.deleted_at)?;
        self.execute(
            "INSERT INTO items
                 (id, title, project_id, block, repeat, weekdays, start_day, planned_minutes,
                  position, created_at, updated_at, deleted_at, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
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
                 version = excluded.version",
            params![
                item.id,
                item.title,
                item.project_id,
                codec::name(&item.block),
                codec::name(&item.repeat),
                item.weekdays.map(|days| days.0),
                codec::day(item.day),
                item.planned_minutes,
                item.position,
                codec::timestamp(item.created_at),
                codec::timestamp(item.updated_at),
                item.deleted_at.map(codec::timestamp),
                version,
            ],
        )?;
        self.log_change("item", &item.id, op, version)
    }

    pub fn put_session(&self, session: &Session) -> Result<i64, Error> {
        let (version, op) = self.next_version("sessions", &session.id, session.deleted_at)?;
        self.execute(
            "INSERT INTO sessions
                 (id, item_id, project_id, day, block, started_at, ended_at, minutes, note,
                  created_at, updated_at, deleted_at, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT (id) DO UPDATE SET
                 item_id = excluded.item_id,
                 project_id = excluded.project_id,
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

    /// Replaces a day's block snapshot. Snapshots are derived data, so they
    /// neither bump the revision nor appear in the change log.
    pub fn replace_day_blocks(&self, day: NaiveDate, blocks: &[DayBlock]) -> Result<(), Error> {
        self.execute("DELETE FROM day_blocks WHERE day = ?1", [codec::day(day)])?;
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
        Ok(())
    }

    /// Replaces a day's plan snapshot; derived data like `replace_day_blocks`.
    pub fn replace_day_plan(&self, day: NaiveDate, lines: &[DayPlanLine]) -> Result<(), Error> {
        self.execute("DELETE FROM day_plan WHERE day = ?1", [codec::day(day)])?;
        for line in lines {
            self.execute(
                "INSERT INTO day_plan (day, block, item_id, planned_minutes)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    codec::day(line.day),
                    codec::name(&line.block),
                    line.item_id,
                    line.planned_minutes,
                ],
            )?;
        }
        Ok(())
    }

    fn execute(&self, sql: &str, params: impl rusqlite::Params) -> Result<(), Error> {
        self.store
            .conn
            .prepare_cached(sql)
            .and_then(|mut statement| statement.execute(params))
            .map(drop)
            .map_err(storage_error)
    }

    /// The version an upsert writes and the change operation it represents.
    fn next_version(
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

    fn log_change(&self, entity: &str, id: &str, op: &str, version: i64) -> Result<i64, Error> {
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
