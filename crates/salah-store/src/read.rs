//! Entity queries. Reads return soft-deleted rows too; callers filter with
//! the domain's `is_live` rules.

use crate::{
    Store,
    codec::{self, storage_error},
};
use chrono::NaiveDate;
use rusqlite::{OptionalExtension, Params, Row};
use salah_domain::{
    error::Error,
    item::PrayerMark,
    tracking::{
        DayBlock, DayPlanLine, Item, Metric, Project, ProjectMetric, Session, SessionValue, Tag,
    },
};

impl Store {
    pub(crate) fn all<T>(
        &self,
        sql: &str,
        params: impl Params,
        map: impl FnMut(&Row) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>, Error> {
        let mut statement = self.conn.prepare_cached(sql).map_err(storage_error)?;
        statement
            .query_map(params, map)
            .map_err(storage_error)?
            .collect::<rusqlite::Result<_>>()
            .map_err(storage_error)
    }

    pub(crate) fn one<T>(
        &self,
        sql: &str,
        params: impl Params,
        map: impl FnOnce(&Row) -> rusqlite::Result<T>,
    ) -> Result<Option<T>, Error> {
        self.conn
            .query_row(sql, params, map)
            .optional()
            .map_err(storage_error)
    }

    pub fn project(&self, id: &str) -> Result<Option<Project>, Error> {
        let project = self.one("SELECT * FROM projects WHERE id = ?1", [id], codec::project)?;
        project.map(|project| self.with_tags(project)).transpose()
    }

    pub fn projects(&self) -> Result<Vec<Project>, Error> {
        self.all(
            "SELECT * FROM projects ORDER BY created_at, id",
            [],
            codec::project,
        )?
        .into_iter()
        .map(|project| self.with_tags(project))
        .collect()
    }

    fn with_tags(&self, mut project: Project) -> Result<Project, Error> {
        project.tag_ids = self.all(
            "SELECT tag_id FROM project_tags WHERE project_id = ?1 ORDER BY tag_id",
            [&project.id],
            |row| row.get(0),
        )?;
        Ok(project)
    }

    pub fn project_metrics(&self, project_id: &str) -> Result<Vec<ProjectMetric>, Error> {
        self.all(
            "SELECT * FROM project_metrics WHERE project_id = ?1 ORDER BY position",
            [project_id],
            codec::project_metric,
        )
    }

    pub fn tag(&self, id: &str) -> Result<Option<Tag>, Error> {
        self.one("SELECT * FROM tags WHERE id = ?1", [id], codec::tag)
    }

    pub fn tags(&self) -> Result<Vec<Tag>, Error> {
        self.all("SELECT * FROM tags ORDER BY name, id", [], codec::tag)
    }

    pub fn metric(&self, id: &str) -> Result<Option<Metric>, Error> {
        self.one("SELECT * FROM metrics WHERE id = ?1", [id], codec::metric)
    }

    /// Built-in metrics first, then by name.
    pub fn metrics(&self) -> Result<Vec<Metric>, Error> {
        self.all(
            "SELECT * FROM metrics ORDER BY builtin DESC, name, id",
            [],
            codec::metric,
        )
    }

    pub fn item(&self, id: &str) -> Result<Option<Item>, Error> {
        self.one("SELECT * FROM items WHERE id = ?1", [id], codec::item)
    }

    pub fn items(&self) -> Result<Vec<Item>, Error> {
        self.all(
            "SELECT * FROM items ORDER BY created_at, id",
            [],
            codec::item,
        )
    }

    pub fn session(&self, id: &str) -> Result<Option<Session>, Error> {
        let session = self.one("SELECT * FROM sessions WHERE id = ?1", [id], codec::session)?;
        session.map(|session| self.with_values(session)).transpose()
    }

    /// Sessions recorded on the days `first..=last`, oldest first.
    pub fn sessions_between(
        &self,
        first: NaiveDate,
        last: NaiveDate,
    ) -> Result<Vec<Session>, Error> {
        self.sessions_where(
            "SELECT * FROM sessions WHERE day BETWEEN ?1 AND ?2 ORDER BY day, created_at, id",
            [codec::day(first), codec::day(last)],
        )
    }

    /// Live sessions with a start and no end.
    pub fn running_sessions(&self) -> Result<Vec<Session>, Error> {
        self.sessions_where(
            "SELECT * FROM sessions
             WHERE started_at IS NOT NULL AND ended_at IS NULL AND deleted_at IS NULL
             ORDER BY started_at, id",
            [],
        )
    }

    fn sessions_where(&self, sql: &str, params: impl Params) -> Result<Vec<Session>, Error> {
        self.all(sql, params, codec::session)?
            .into_iter()
            .map(|session| self.with_values(session))
            .collect()
    }

    fn with_values(&self, mut session: Session) -> Result<Session, Error> {
        session.values = self.all(
            "SELECT metric_id, value FROM session_values WHERE session_id = ?1 ORDER BY metric_id",
            [&session.id],
            |row| {
                Ok(SessionValue {
                    metric_id: row.get("metric_id")?,
                    value: row.get("value")?,
                })
            },
        )?;
        Ok(session)
    }

    pub fn day_blocks(&self, day: NaiveDate) -> Result<Vec<DayBlock>, Error> {
        self.all(
            "SELECT * FROM day_blocks WHERE day = ?1 ORDER BY starts_at",
            [codec::day(day)],
            codec::day_block,
        )
    }

    pub fn day_plan(&self, day: NaiveDate) -> Result<Vec<DayPlanLine>, Error> {
        self.all(
            "SELECT * FROM day_plan WHERE day = ?1 ORDER BY block, item_id",
            [codec::day(day)],
            codec::day_plan_line,
        )
    }

    pub fn mark(&self, id: &str) -> Result<Option<PrayerMark>, Error> {
        self.one(
            "SELECT * FROM prayer_marks WHERE id = ?1",
            [id],
            codec::mark,
        )
    }

    pub fn marks_between(
        &self,
        first: NaiveDate,
        last: NaiveDate,
    ) -> Result<Vec<PrayerMark>, Error> {
        self.all(
            "SELECT * FROM prayer_marks WHERE day BETWEEN ?1 AND ?2 ORDER BY day, id",
            [codec::day(first), codec::day(last)],
            codec::mark,
        )
    }
}
