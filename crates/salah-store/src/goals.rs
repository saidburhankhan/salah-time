use crate::{Store, WriteTx, codec, reports::ASSIGNED_SESSIONS};
use chrono::NaiveDate;
use rusqlite::{Row, params};
use salah_domain::{
    error::Error,
    goal::{Goal, Scope},
};

fn goal(row: &Row) -> rusqlite::Result<Goal> {
    Ok(Goal {
        id: row.get("id")?,
        scope: Scope {
            project_id: row.get("project_id")?,
            epic_id: row.get("epic_id")?,
            item_id: row.get("item_id")?,
        },
        period: codec::get_name(row, "period")?,
        metric_id: row.get("metric_id")?,
        direction: codec::get_name(row, "direction")?,
        target: row.get("target")?,
        starts_on: codec::get_day(row, "starts_on")?,
        ends_on: codec::get_optional_day(row, "ends_on")?,
        replaces_id: row.get("replaces_id")?,
        position: row.get("position")?,
        created_at: codec::get_timestamp(row, "created_at")?,
        updated_at: codec::get_timestamp(row, "updated_at")?,
        deleted_at: codec::get_optional_timestamp(row, "deleted_at")?,
        version: row.get("version")?,
    })
}

fn sample_query(scope: &Scope, has_metric: bool) -> (String, Option<&String>) {
    let (column, owner) = if let Some(id) = &scope.item_id {
        ("s.item_id", Some(id))
    } else if let Some(id) = &scope.epic_id {
        ("s.epic_id", Some(id))
    } else if let Some(id) = &scope.project_id {
        ("s.project_id", Some(id))
    } else {
        ("NULL", None)
    };
    // CROSS JOIN fixes SQLite's outer loop on the matched sessions. Each
    // metric lookup can then use its (session_id, metric_id) primary key.
    let (source, value) = if has_metric {
        (
            "CROSS JOIN session_values v ON v.session_id=s.id AND v.metric_id=?4",
            "v.value",
        )
    } else {
        ("", "COALESCE(s.minutes,0)")
    };
    let sql = format!(
        "SELECT s.day, {value}, MAX(COALESCE(s.minutes, 1), 1) FROM {ASSIGNED_SESSIONS} s {source}
            WHERE s.day BETWEEN ?1 AND ?2 AND {column} IS ?3 AND s.deleted_at IS NULL
            AND (s.started_at IS NULL OR s.ended_at IS NOT NULL)"
    );
    (sql, owner)
}

impl Store {
    pub fn goal(&self, id: &str) -> Result<Option<Goal>, Error> {
        self.one("SELECT * FROM goals WHERE id=?1", [id], goal)
    }

    /// Use the stored name even after an item, epic or project is soft-deleted.
    pub fn goal_scope_label(&self, scope: &Scope) -> Result<Option<String>, Error> {
        let (sql, id) = if let Some(id) = &scope.item_id {
            ("SELECT title FROM items WHERE id=?1", id)
        } else if let Some(id) = &scope.epic_id {
            ("SELECT name FROM epics WHERE id=?1", id)
        } else if let Some(id) = &scope.project_id {
            ("SELECT name FROM projects WHERE id=?1", id)
        } else {
            return Ok(None);
        };
        self.one(sql, [id], |row| row.get(0))
    }

    pub fn goals_at(&self, at: NaiveDate, include_ended: bool) -> Result<Vec<Goal>, Error> {
        self.all(
            "SELECT * FROM goals WHERE deleted_at IS NULL AND starts_on<=?1
            AND (?2 OR ends_on IS NULL OR ends_on>=?1) ORDER BY position,starts_on,id",
            params![codec::day(at), include_ended],
            goal,
        )
    }

    pub fn goals_between(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<Goal>, Error> {
        self.all("SELECT * FROM goals WHERE deleted_at IS NULL AND starts_on<=?2 AND (ends_on IS NULL OR ends_on>=?1) ORDER BY position,starts_on,id",
            params![codec::day(from),codec::day(to)],goal)
    }

    pub fn goals_for_scope(&self, scope: &Scope) -> Result<Vec<Goal>, Error> {
        self.all(
            "SELECT * FROM goals WHERE deleted_at IS NULL AND ends_on IS NULL AND
            project_id IS ?1 AND epic_id IS ?2 AND item_id IS ?3 ORDER BY position,id",
            params![scope.project_id, scope.epic_id, scope.item_id],
            goal,
        )
    }

    pub fn goal_history_versions(&self, id: &str) -> Result<Vec<Goal>, Error> {
        self.all(
            "WITH RECURSIVE family(id) AS (SELECT id FROM goals WHERE id = ?1
                UNION SELECT g.id FROM goals g JOIN family f ON g.replaces_id = f.id
                UNION SELECT g.replaces_id FROM goals g JOIN family f ON g.id = f.id
                    WHERE g.replaces_id IS NOT NULL)
             SELECT g.* FROM goals g JOIN family f ON f.id = g.id
             WHERE g.deleted_at IS NULL ORDER BY g.starts_on, g.id",
            [id],
            goal,
        )
    }

    /// `(day, value, weight)` of the finished live sessions in a goal's scope,
    /// the same sessions `stats.*` reads. Minutes goals read each session's
    /// minutes (ticks count 0); metric goals read only sessions that recorded
    /// the metric, weighted by minutes like stats (a tick or 0-minute session
    /// weighs 1). An all-work goal has no scope column, so work without a
    /// project counts only there.
    pub fn goal_samples(
        &self,
        g: &Goal,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<(NaiveDate, f64, f64)>, Error> {
        let (sql, owner) = sample_query(&g.scope, g.metric_id.is_some());
        let mut args = vec![Some(codec::day(from)), Some(codec::day(to)), owner.cloned()];
        if let Some(id) = &g.metric_id {
            args.push(Some(id.clone()));
        }
        self.all(&sql, rusqlite::params_from_iter(args), |r| {
            Ok((codec::get_day(r, "day")?, r.get(1)?, r.get(2)?))
        })
    }

    pub fn next_goal_position(&self) -> Result<i64, Error> {
        self.conn
            .query_row("SELECT COALESCE(MAX(position)+1,0) FROM goals", [], |r| {
                r.get(0)
            })
            .map_err(codec::storage_error)
    }

    fn validate_goal_scope(&self, g: &Goal) -> Result<(), Error> {
        let Scope {
            project_id,
            epic_id,
            item_id,
        } = &g.scope;
        if [project_id, epic_id, item_id]
            .iter()
            .filter(|s| s.is_some())
            .count()
            > 1
        {
            return Err(Error::validation("scope", "choose at most one goal scope"));
        }
        let mut project = project_id.clone();
        let mut selection = None;
        if let Some(id) = item_id {
            let item = self
                .item(id)?
                .filter(|i| i.is_live())
                .ok_or_else(|| Error::NotFound {
                    what: "item",
                    id: id.clone(),
                })?;
            project = item.project_id;
            selection = item.metric_ids;
        } else if let Some(id) = epic_id {
            project = Some(
                self.epic(id)?
                    .filter(|e| e.deleted_at.is_none())
                    .ok_or_else(|| Error::NotFound {
                        what: "epic",
                        id: id.clone(),
                    })?
                    .project_id,
            );
        } else if let Some(id) = project_id
            && self.project(id)?.is_none_or(|p| p.deleted_at.is_some())
        {
            return Err(Error::NotFound {
                what: "project",
                id: id.clone(),
            });
        }
        if let Some(id) = &g.metric_id {
            let metric = self
                .metric(id)?
                .filter(|m| m.deleted_at.is_none())
                .ok_or_else(|| Error::NotFound {
                    what: "metric",
                    id: id.clone(),
                })?;
            let asked = match (selection, project) {
                (Some(ids), _) => ids.contains(id),
                (_, Some(p)) => self.project_metrics(&p)?.iter().any(|m| m.metric_id == *id),
                _ => metric.builtin,
            };
            if metric.archived || !asked {
                return Err(Error::validation(
                    "metric_id",
                    "scope does not ask this active metric",
                ));
            }
        }
        Ok(())
    }
}

impl WriteTx<'_> {
    pub fn insert_goal(&self, g: &Goal) -> Result<i64, Error> {
        self.validate_goal_scope(g)?;
        if !g.target.is_finite() || g.target <= 0.0 {
            return Err(Error::validation(
                "target",
                "target must be finite and positive",
            ));
        }
        if g.metric_id.is_none() && g.direction != salah_domain::goal::Direction::AtLeast {
            return Err(Error::validation(
                "direction",
                "minutes goals must use at_least",
            ));
        }
        if g.period.bounds(g.starts_on)?.0 != g.starts_on {
            return Err(Error::validation(
                "starts_on",
                "goal must start at the period boundary",
            ));
        }
        self.execute(
            "INSERT INTO goals (id, project_id, epic_id, item_id, period, metric_id, direction,
                target, starts_on, replaces_id, position, created_at, updated_at, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, 1)",
            params![
                g.id,
                g.scope.project_id,
                g.scope.epic_id,
                g.scope.item_id,
                codec::name(&g.period),
                g.metric_id,
                codec::name(&g.direction),
                g.target,
                codec::day(g.starts_on),
                g.replaces_id,
                g.position,
                codec::timestamp(g.created_at)
            ],
        )?;
        self.log_change("goals", &g.id, "insert", 1)
    }

    /// Closing a goal only changes its end/deletion and sync fields.
    pub fn close_goal(&self, g: &Goal) -> Result<i64, Error> {
        if let Some(end) = g.ends_on
            && (g.period.bounds(end)?.1 != end || end < g.starts_on)
        {
            return Err(Error::validation(
                "ends_on",
                "goal must end at a valid period boundary",
            ));
        }
        let (version, op) = self.next_version("goals", &g.id, g.deleted_at)?;
        self.execute(
            "UPDATE goals SET ends_on=?2,deleted_at=?3,updated_at=?4,version=?5 WHERE id=?1",
            params![
                g.id,
                g.ends_on.map(codec::day),
                g.deleted_at.map(codec::timestamp),
                codec::timestamp(g.updated_at),
                version
            ],
        )?;
        self.log_change("goals", &g.id, op, version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_goal_queries_probe_values_after_period_and_scope_selection() {
        let store = Store::open_in_memory().unwrap();
        for scope in [
            Scope::default(),
            Scope {
                item_id: Some("task".into()),
                ..Default::default()
            },
            Scope {
                project_id: Some("project".into()),
                ..Default::default()
            },
            Scope {
                epic_id: Some("epic".into()),
                ..Default::default()
            },
        ] {
            let (sql, owner) = sample_query(&scope, true);
            let plan = store
                .all(
                    &format!("EXPLAIN QUERY PLAN {sql}"),
                    params!["2031-05-12", "2031-05-18", owner, "focus"],
                    |r| r.get::<_, String>(3),
                )
                .unwrap();
            let session_index = if scope.item_id.is_some() {
                "sessions_by_item_day"
            } else {
                "sessions_by_day"
            };
            let outer = plan
                .iter()
                .position(|p| {
                    p.contains(session_index) && p.contains("day>?") && p.contains("day<?")
                })
                .expect("bounded session index search");
            let probe = plan
                .iter()
                .position(|p| p.contains("session_id=? AND metric_id=?"))
                .expect("compound metric lookup");
            assert!(outer < probe, "{plan:?}");
            for (required, lookup) in [
                (
                    scope.project_id.is_some() || scope.epic_id.is_some(),
                    "SEARCH i ",
                ),
                (scope.epic_id.is_some(), "SEARCH e "),
            ] {
                if required {
                    assert!(
                        plan[outer + 1..probe].iter().any(|p| p.starts_with(lookup)),
                        "{plan:?}"
                    );
                }
            }
            assert!(
                !plan.iter().any(|p| p.contains("session_values_by_metric")),
                "{plan:?}"
            );
        }
    }
}
