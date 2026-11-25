//! Tracking-day archive spans, catalogue counts and the work to recover on resume.
use crate::{Store, WriteTx, codec};
use chrono::NaiveDate;
use rusqlite::params;
use salah_domain::{
    error::Error,
    goal::{GoalPause, PauseProject, Scope},
    tracking::PausePreview,
};
use std::collections::BTreeSet;

const DONE: &str = "EXISTS(SELECT 1 FROM sessions s WHERE s.item_id=i.id AND s.deleted_at IS NULL
    AND s.minutes IS NULL AND (s.started_at IS NULL OR s.ended_at IS NOT NULL))";

impl Store {
    pub fn item_completed_by(&self, id: &str, day: NaiveDate) -> Result<bool, Error> {
        let sql = "SELECT EXISTS(SELECT 1 FROM sessions WHERE item_id=?1 AND day<=?2
                   AND deleted_at IS NULL AND minutes IS NULL AND (started_at IS NULL OR ended_at IS NOT NULL))";
        self.conn
            .query_row(sql, params![id, codec::day(day)], |r| r.get(0))
            .map_err(codec::storage_error)
    }

    pub fn project_archived(&self, id: Option<&str>) -> Result<bool, Error> {
        match id {
            Some(id) => Ok(self
                .one("SELECT archived FROM projects WHERE id=?1", [id], |r| {
                    r.get(0)
                })?
                .unwrap_or(false)),
            None => Ok(false),
        }
    }

    pub fn paused_projects_on(&self, day: NaiveDate) -> Result<BTreeSet<String>, Error> {
        let sql = "SELECT project_id FROM project_pauses
                   WHERE from_day<=?1 AND (to_day IS NULL OR to_day>=?1)";
        Ok(self
            .all(sql, [codec::day(day)], |r| r.get(0))?
            .into_iter()
            .collect())
    }

    /// Adherence also needs the pause state of tasks that had no scheduled occurrence.
    pub fn paused_items_on(&self, day: NaiveDate) -> Result<BTreeSet<String>, Error> {
        let sql = "WITH paused AS MATERIALIZED (
                     SELECT project_id FROM project_pauses WHERE from_day<=?1 AND (to_day IS NULL OR to_day>=?1))
                   SELECT i.id FROM items i
                   LEFT JOIN item_history h ON h.item_id=i.id AND h.until_day=(
                     SELECT min(until_day) FROM item_history WHERE item_id=i.id AND until_day>?1)
                   WHERE EXISTS (SELECT 1 FROM paused)
                     AND (CASE WHEN h.item_id IS NULL THEN i.project_id
                               ELSE json_extract(h.item_json,'$.project_id') END) IN (SELECT project_id FROM paused)";
        Ok(self
            .all(sql, [codec::day(day)], |r| r.get(0))?
            .into_iter()
            .collect())
    }

    pub fn pause_preview(&self, id: &str, today: NaiveDate) -> Result<PausePreview, Error> {
        let sql = format!(
            "SELECT (SELECT count(*) FROM goals g
                       WHERE g.deleted_at IS NULL AND g.ends_on IS NULL AND g.starts_on<=?2
                         AND (g.project_id=p.id OR g.epic_id IN (SELECT id FROM epics WHERE project_id=p.id)
                              OR g.item_id IN (SELECT id FROM items WHERE project_id=p.id))),
                    (SELECT count(*) FROM items i WHERE i.project_id=p.id AND i.deleted_at IS NULL
                         AND i.repeat!='none' AND (i.end_day IS NULL OR i.end_day>=?2)),
                    (SELECT count(*) FROM items i WHERE i.project_id=p.id AND i.deleted_at IS NULL
                         AND i.repeat='none' AND NOT {DONE})
             FROM projects p WHERE p.id=?1 AND p.deleted_at IS NULL"
        );
        self.one(&sql, params![id, codec::day(today)], |r| {
            Ok(PausePreview {
                goals: r.get(0)?,
                routines: r.get(1)?,
                open_tasks: r.get(2)?,
            })
        })?
        .ok_or_else(|| Error::NotFound {
            what: "project",
            id: id.to_owned(),
        })
    }

    /// Project assignment follows the task's schedule history when evaluating a past goal.
    pub fn goal_pauses(
        &self,
        scope: &Scope,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<GoalPause>, Error> {
        let (owner, ownership) = if let Some(id) = &scope.item_id {
            (id, "SELECT project_id, lag(until_day) OVER (ORDER BY until_day NULLS LAST) AS first,
              date(until_day,'-1 day') AS last FROM (
              SELECT json_extract(item_json,'$.project_id') AS project_id, until_day FROM item_history WHERE item_id=?1
              UNION ALL SELECT project_id,NULL FROM items WHERE id=?1)")
        } else if let Some(id) = &scope.epic_id {
            (
                id,
                "SELECT project_id,NULL AS first,NULL AS last FROM epics WHERE id=?1",
            )
        } else if let Some(id) = &scope.project_id {
            (id, "SELECT ?1 AS project_id,NULL AS first,NULL AS last")
        } else {
            return Ok(vec![]);
        };
        let sql = format!(
            "WITH owners AS ({ownership}), spans AS (
                SELECT p.id, p.name, max(a.from_day, coalesce(o.first,?2), ?2) AS first,
                       min(coalesce(a.to_day,?3), coalesce(o.last,?3), ?3) AS last
                  FROM owners o JOIN project_pauses a ON a.project_id=o.project_id
                  JOIN projects p ON p.id=a.project_id
                 WHERE a.from_day<=?3 AND (a.to_day IS NULL OR a.to_day>=?2))
             SELECT * FROM spans WHERE first<=last ORDER BY first,id"
        );
        self.all(
            &sql,
            params![owner, codec::day(from), codec::day(to)],
            |r| {
                Ok(GoalPause {
                    project: PauseProject {
                        id: r.get("id")?,
                        name: r.get("name")?,
                    },
                    from: codec::get_day(r, "first")?,
                    to: codec::get_day(r, "last")?,
                })
            },
        )
    }

    pub fn expired_task_ids(
        &self,
        id: &str,
        from: NaiveDate,
        resume: NaiveDate,
    ) -> Result<Vec<String>, Error> {
        let sql = format!(
            "SELECT i.id FROM items i WHERE i.project_id=?1 AND i.deleted_at IS NULL AND i.repeat='none'
               AND i.start_day>=?2 AND i.start_day<?3 AND NOT {DONE} ORDER BY start_day,block,position,id"
        );
        self.all(
            &sql,
            params![id, codec::day(from), codec::day(resume)],
            |r| r.get(0),
        )
    }
}

impl WriteTx<'_> {
    pub fn start_pause(&self, id: &str, day: NaiveDate) -> Result<(), Error> {
        self.execute(
            "INSERT INTO project_pauses(project_id,from_day) VALUES(?1,?2)",
            params![id, codec::day(day)],
        )
    }

    pub fn finish_pause(&self, id: &str, day: NaiveDate) -> Result<NaiveDate, Error> {
        let start = self
            .one(
                "SELECT from_day FROM project_pauses WHERE project_id=?1 AND to_day IS NULL",
                [id],
                |r| codec::get_day(r, "from_day"),
            )?
            .ok_or_else(|| Error::Storage("archived project has no open pause".into()))?;
        if day <= start {
            self.execute(
                "DELETE FROM project_pauses WHERE project_id=?1 AND to_day IS NULL",
                [id],
            )?;
        } else {
            self.execute(
                "UPDATE project_pauses SET to_day=?2 WHERE project_id=?1 AND to_day IS NULL",
                params![id, codec::day(day.pred_opt().unwrap())],
            )?;
        }
        Ok(start)
    }
}
