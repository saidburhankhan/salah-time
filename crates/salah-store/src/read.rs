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
        DayBlock, DayClock, DayPlanLine, Item, Metric, Project, ProjectMetric, Session,
        SessionNote, SessionValue, Tag,
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

    /// Keep the original built-ins first, followed by distractions and named custom metrics.
    pub fn metrics(&self) -> Result<Vec<Metric>, Error> {
        self.all(
            "SELECT * FROM metrics ORDER BY builtin DESC, (id = 'distractions'), name, id",
            [],
            codec::metric,
        )
    }

    pub fn item(&self, id: &str) -> Result<Option<Item>, Error> {
        self.one("SELECT * FROM items WHERE id = ?1", [id], codec::item)?
            .map(|item| self.with_metric_ids(item))
            .transpose()
    }

    pub fn items_in_epic(&self, id: &str) -> Result<Vec<Item>, Error> {
        self.items_where(
            "SELECT * FROM items WHERE epic_id=?1 AND deleted_at IS NULL ORDER BY id",
            [id],
        )
    }

    pub fn items(&self) -> Result<Vec<Item>, Error> {
        self.items_where("SELECT * FROM items ORDER BY created_at, id", [])
    }

    pub fn custom_items_in_project(&self, project_id: &str) -> Result<Vec<Item>, Error> {
        self.items_where(
            "SELECT * FROM items WHERE project_id = ?1 AND metrics_custom = 1 ORDER BY id",
            [project_id],
        )
    }

    fn items_where(&self, sql: &str, params: impl Params) -> Result<Vec<Item>, Error> {
        self.all(sql, params, codec::item)?
            .into_iter()
            .map(|item| self.with_metric_ids(item))
            .collect()
    }

    fn with_metric_ids(&self, mut item: Item) -> Result<Item, Error> {
        if let Some(ids) = &mut item.metric_ids {
            *ids = self.all(
                "SELECT metric_id FROM item_metrics WHERE item_id = ?1 ORDER BY position",
                [&item.id],
                |row| row.get(0),
            )?;
        }
        Ok(item)
    }

    /// Item state at the close of a past day, including tombstones.
    pub fn items_on(&self, day: NaiveDate) -> Result<Vec<Item>, Error> {
        let mut items = self.items()?;
        let bodies: Vec<String> = self.all(
            "SELECT h.item_json FROM item_history h WHERE h.until_day = (
                SELECT MIN(v.until_day) FROM item_history v
                WHERE v.item_id = h.item_id AND v.until_day > ?1)",
            [codec::day(day)],
            |row| row.get(0),
        )?;
        let mut previous = std::collections::HashMap::new();
        for body in bodies {
            let item: Item =
                serde_json::from_str(&body).map_err(|e| Error::Storage(e.to_string()))?;
            previous.insert(item.id.clone(), item);
        }
        for item in &mut items {
            if let Some(old) = previous.remove(&item.id) {
                // The editable entity still needs its current version as a precondition.
                *item = Item {
                    version: item.version,
                    ..old
                };
            }
        }
        Ok(items)
    }

    pub fn historical_config(&self, day: NaiveDate) -> Result<Option<String>, Error> {
        self.one("SELECT config_json FROM config_history WHERE until_day > ?1 ORDER BY until_day LIMIT 1",
            [codec::day(day)], |row| row.get(0))
    }

    pub fn day_clock(&self, day: NaiveDate) -> Result<Option<DayClock>, Error> {
        self.one(
            "SELECT * FROM day_clocks WHERE day = ?1",
            [codec::day(day)],
            |row| {
                Ok(DayClock {
                    offset_seconds: row.get("offset_seconds")?,
                    sunrise_minutes: row.get("sunrise_minutes")?,
                    sleep_at: codec::get_timestamp(row, "sleep_at")?,
                })
            },
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
        self.sessions_between_for_item(first, last, None)
    }

    pub fn sessions_between_for_item(
        &self,
        first: NaiveDate,
        last: NaiveDate,
        item_id: Option<&str>,
    ) -> Result<Vec<Session>, Error> {
        if let Some(id) = item_id {
            return self.sessions_where(
                "SELECT * FROM sessions WHERE item_id = ?1 AND day BETWEEN ?2 AND ?3
                 ORDER BY day, created_at, id",
                rusqlite::params![id, codec::day(first), codec::day(last)],
            );
        }
        self.sessions_where(
            "SELECT * FROM sessions WHERE day BETWEEN ?1 AND ?2 ORDER BY day, created_at, id",
            [codec::day(first), codec::day(last)],
        )
    }

    pub fn session_notes(&self, session_id: &str) -> Result<Vec<SessionNote>, Error> {
        self.all(
            "SELECT id, at, text FROM session_notes WHERE session_id = ?1 AND deleted_at IS NULL
             ORDER BY at, rowid",
            [session_id],
            |row| {
                Ok(SessionNote {
                    id: row.get("id")?,
                    at: codec::get_timestamp(row, "at")?,
                    text: row.get("text")?,
                })
            },
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

    /// Whether the snapshot of `day` was written.
    pub fn day_written(&self, day: NaiveDate) -> Result<bool, Error> {
        Ok(self
            .one(
                "SELECT 1 FROM days_written WHERE day = ?1",
                [codec::day(day)],
                |_| Ok(()),
            )?
            .is_some())
    }

    pub fn written_days(
        &self,
        from: NaiveDate,
        until: NaiveDate,
    ) -> Result<std::collections::BTreeSet<NaiveDate>, Error> {
        let days = self.all(
            "SELECT day FROM days_written WHERE day >= ?1 AND day < ?2",
            [codec::day(from), codec::day(until)],
            |row| codec::get_day(row, "day"),
        )?;
        Ok(days.into_iter().collect())
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
