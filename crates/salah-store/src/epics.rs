use crate::{Store, codec};
use chrono::NaiveDate;
use rusqlite::params;
use salah_domain::{
    epic::{Epic, EpicStatus, EpicView},
    error::Error,
};

impl Store {
    pub fn epic(&self, id: &str) -> Result<Option<Epic>, Error> {
        self.one("SELECT * FROM epics WHERE id=?1", [id], codec::epic)
    }

    pub fn next_epic_position(&self, project_id: &str) -> Result<i64, Error> {
        self.conn
            .query_row(
                "SELECT COALESCE(MAX(position)+1,0) FROM epics WHERE project_id=?1",
                [project_id],
                |row| row.get(0),
            )
            .map_err(codec::storage_error)
    }

    pub fn epic_has_sessions(&self, id: &str) -> Result<bool, Error> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE epic_id=?1 AND deleted_at IS NULL)",
                [id],
                |row| row.get(0),
            )
            .map_err(codec::storage_error)
    }

    pub fn epic_views(
        &self,
        project: Option<&str>,
        status: Option<EpicStatus>,
        today: NaiveDate,
    ) -> Result<Vec<EpicView>, Error> {
        self.all(
            "WITH selected AS (
                 SELECT * FROM epics WHERE deleted_at IS NULL AND (?1 IS NULL OR project_id=?1)
                     AND (?2 IS NULL OR status=?2)
             ), work AS (
                 SELECT s.epic_id, SUM(s.minutes) AS minutes, COUNT(*) AS sessions,
                        COUNT(DISTINCT s.day) AS days, MAX(s.day) AS last_day
                 FROM sessions s JOIN selected e ON e.id=s.epic_id
                 WHERE s.deleted_at IS NULL AND (s.started_at IS NULL OR s.ended_at IS NOT NULL)
                 GROUP BY s.epic_id
             ), tasks AS (
                 SELECT i.epic_id, COUNT(*) AS total, SUM(EXISTS(
                     SELECT 1 FROM sessions s WHERE s.item_id=i.id AND s.deleted_at IS NULL
                         AND s.minutes IS NULL AND (s.started_at IS NULL OR s.ended_at IS NOT NULL)
                         AND (i.repeat='none' OR s.day=?3)
                 )) AS done FROM items i JOIN selected e ON e.id=i.epic_id
                 WHERE i.deleted_at IS NULL GROUP BY i.epic_id
             )
             SELECT e.*, COALESCE(w.minutes,0) AS total_minutes, COALESCE(w.sessions,0) AS sessions,
                    COALESCE(w.days,0) AS days_worked, w.last_day,
                    COALESCE(t.total-t.done,0) AS open_tasks, COALESCE(t.done,0) AS done_tasks
             FROM selected e LEFT JOIN work w ON e.id=w.epic_id LEFT JOIN tasks t ON e.id=t.epic_id
             ORDER BY e.project_id,e.position,e.id",
            params![project, status.map(|s| codec::name(&s)), codec::day(today)],
            |row| {
                Ok(EpicView {
                    epic: codec::epic(row)?,
                    total_minutes: row.get("total_minutes")?,
                    sessions: row.get("sessions")?,
                    days_worked: row.get("days_worked")?,
                    last_worked: if row.get::<_, Option<String>>("last_day")?.is_none() {
                        None
                    } else {
                        Some(codec::get_day(row, "last_day")?)
                    },
                    open_tasks: row.get("open_tasks")?,
                    done_tasks: row.get("done_tasks")?,
                })
            },
        )
    }
}
