//! Finished-session queries shared by the stats table and the Notes page.
//! Every filter runs in SQL; only the matched sessions are materialised.

use crate::{
    Store,
    codec::{self, storage_error},
};
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{Connection, functions::FunctionFlags, params_from_iter, types::Value};
use salah_domain::{
    error::Error,
    tracking::{ItemRef, ProjectRef, Session, SessionNote, SessionValue, Tag},
};
use std::collections::BTreeMap;

/// One filter dimension: a specific id, or work assigned to none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice<'a> {
    Unassigned,
    Is(&'a str),
}

/// The days and assignment filters every report shares. The filters intersect.
#[derive(Clone, Debug)]
pub struct SessionScope<'a> {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub project: Option<Choice<'a>>,
    pub epic: Option<Choice<'a>>,
    /// `Unassigned`: the session's project has no live tag (or there is no project).
    pub tag: Option<Choice<'a>>,
    pub item: Option<Choice<'a>>,
    /// Duration and metric bounds on the sessions themselves.
    pub measures: SessionFilter<'a>,
}

/// Inclusive bounds on one metric value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

/// Predicates on a session's own minutes and recorded metric values, shared
/// by every report that narrows by them.
#[derive(Clone, Debug, Default)]
pub struct SessionFilter<'a> {
    pub minutes_min: Option<i64>,
    pub minutes_max: Option<i64>,
    /// A session lacking a listed metric does not match.
    pub values: BTreeMap<&'a str, Bounds>,
}

/// The Notes page's extra predicates and its newest-first page.
#[derive(Clone, Debug, Default)]
pub struct SessionPage<'a> {
    /// The session has a main note or at least one live timestamped entry.
    pub with_note: bool,
    /// Lowercased text matched against the main note and every live entry.
    pub q: Option<&'a str>,
    /// Rows strictly after this `(day, instant, id)` cursor in page order.
    pub before: Option<(NaiveDate, DateTime<Utc>, &'a str)>,
    pub fetch: usize,
}

/// A finished session with the labels of its recorded project and item.
#[derive(Clone, Debug)]
pub struct ReportSession {
    pub session: Session,
    pub project: Option<ProjectRef>,
    pub epic_name: Option<String>,
    pub item: Option<ItemRef>,
}

impl ReportSession {
    /// The instant that orders a day's sessions on the Notes page: the end
    /// of a timed session, else (manual logs and ticks) its creation.
    pub fn sort_at(&self) -> DateTime<Utc> {
        self.session.ended_at.unwrap_or(self.session.created_at)
    }
}

/// SQLite's `lower()` only folds ASCII. This registers the same Unicode
/// lowercase conversion used for the query text, so search stays in SQL.
pub(crate) fn register_search(conn: &Connection) -> Result<(), Error> {
    conn.create_scalar_function(
        "salah_lower",
        1,
        FunctionFlags::SQLITE_UTF8
            | FunctionFlags::SQLITE_DETERMINISTIC
            | FunctionFlags::SQLITE_INNOCUOUS,
        |ctx| Ok(ctx.get::<Option<String>>(0)?.map(|s| s.to_lowercase())),
    )
    .map_err(storage_error)
}

/// The ordering instant in the store's own timestamp spelling, so text
/// comparison matches time order even for older spellings.
const SORT_AT: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', COALESCE(s.ended_at, s.created_at))";

/// Newest planning day first, then the newest instant within the day, so a
/// session logged later for an earlier day sorts under that day.
const PAGE_ORDER: &str = "s.day DESC, strftime('%Y-%m-%dT%H:%M:%fZ', COALESCE(s.ended_at, s.created_at)) DESC, s.id DESC";

/// Live sessions with a project tag that is itself live.
const HAS_LIVE_TAG: &str = "EXISTS (SELECT 1 FROM project_tags pt JOIN tags t ON t.id = pt.tag_id
    AND t.deleted_at IS NULL WHERE pt.project_id = s.project_id)";

const HAS_ENTRY: &str = "EXISTS (SELECT 1 FROM session_notes n WHERE n.session_id = s.id
    AND n.deleted_at IS NULL";

struct Query {
    sql: String,
    args: Vec<Value>,
}

impl Query {
    fn scoped(scope: &SessionScope<'_>) -> Self {
        let mut query = Self {
            sql: "SELECT s.* FROM sessions s WHERE s.deleted_at IS NULL
                AND (s.started_at IS NULL OR s.ended_at IS NOT NULL)
                AND s.day BETWEEN ? AND ?"
                .into(),
            args: vec![codec::day(scope.from).into(), codec::day(scope.to).into()],
        };
        match scope.project {
            Some(Choice::Unassigned) => query.push("s.project_id IS NULL", []),
            Some(Choice::Is(id)) => query.push("s.project_id = ?", [text(id)]),
            None => {}
        }
        match scope.tag {
            Some(Choice::Unassigned) => query.push(&format!("NOT {HAS_LIVE_TAG}"), []),
            Some(Choice::Is(id)) => query.push(
                "EXISTS (SELECT 1 FROM project_tags pt WHERE pt.project_id = s.project_id
                 AND pt.tag_id = ?)",
                [text(id)],
            ),
            None => {}
        }
        match scope.epic {
            Some(Choice::Unassigned) => query.push("s.epic_id IS NULL", []),
            Some(Choice::Is(id)) => query.push("s.epic_id = ?", [text(id)]),
            None => {}
        }
        match scope.item {
            Some(Choice::Unassigned) => query.push("s.item_id IS NULL", []),
            Some(Choice::Is(id)) => query.push("s.item_id = ?", [text(id)]),
            None => {}
        }
        query.measures(&scope.measures);
        query
    }

    /// The duration and metric predicates of one [`SessionFilter`].
    fn measures(&mut self, filter: &SessionFilter<'_>) {
        if let Some(min) = filter.minutes_min {
            self.push("s.minutes >= ?", [min.into()]);
        }
        if let Some(max) = filter.minutes_max {
            self.push("s.minutes <= ?", [max.into()]);
        }
        for (metric_id, bounds) in &filter.values {
            self.push(
                "EXISTS (SELECT 1 FROM session_values v WHERE v.session_id = s.id
                 AND v.metric_id = ? AND (? IS NULL OR v.value >= ?) AND (? IS NULL OR v.value <= ?))",
                [
                    text(metric_id),
                    bounds.min.into(),
                    bounds.min.into(),
                    bounds.max.into(),
                    bounds.max.into(),
                ],
            );
        }
    }

    fn push<const N: usize>(&mut self, condition: &str, args: [Value; N]) {
        self.sql.push_str(" AND ");
        self.sql.push_str(condition);
        self.args.extend(args);
    }

    fn page(&mut self, page: &SessionPage<'_>) {
        if let Some(q) = page.q {
            self.push(
                &format!(
                    "(instr(salah_lower(s.note), ?) > 0 OR {HAS_ENTRY}
                     AND instr(salah_lower(n.text), ?) > 0))"
                ),
                [text(q), text(q)],
            );
        } else if page.with_note {
            self.push(&format!("(length(s.note) > 0 OR {HAS_ENTRY}))"), []);
        }
        if let Some((day, at, id)) = page.before {
            self.push(
                &format!("(s.day, {SORT_AT}, s.id) < (?, ?, ?)"),
                [
                    codec::day(day).into(),
                    codec::timestamp(at).into(),
                    text(id),
                ],
            );
        }
        self.sql
            .push_str(&format!(" ORDER BY {PAGE_ORDER} LIMIT ?"));
        self.args.push((page.fetch as i64).into());
    }
}

impl Store {
    /// Finished live sessions in scope with their values and labels: oldest
    /// first by day without a page; with one, newest day first, then newest
    /// [`ReportSession::sort_at`], then id.
    pub fn report_sessions(
        &self,
        scope: &SessionScope<'_>,
        page: Option<&SessionPage<'_>>,
    ) -> Result<Vec<ReportSession>, Error> {
        let mut query = Query::scoped(scope);
        let order = match page {
            Some(page) => {
                query.page(page);
                PAGE_ORDER.to_owned()
            }
            None => "s.day, s.created_at, s.id".into(),
        };
        let sql = format!(
            "WITH matched AS ({}) SELECT s.*, p.name AS project_name, p.color AS project_color,
             e.name AS epic_name, i.title AS item_title, v.metric_id, v.value FROM matched s
             LEFT JOIN projects p ON p.id = s.project_id LEFT JOIN items i ON i.id = s.item_id
             LEFT JOIN epics e ON e.id = s.epic_id
             LEFT JOIN session_values v ON v.session_id = s.id ORDER BY {order}, v.metric_id",
            query.sql
        );
        let mut statement = self.conn.prepare(&sql).map_err(storage_error)?;
        let mut rows = statement
            .query(params_from_iter(query.args))
            .map_err(storage_error)?;
        let mut result: Vec<ReportSession> = Vec::new();
        while let Some(row) = rows.next().map_err(storage_error)? {
            let id: String = row.get("id").map_err(storage_error)?;
            if result.last().is_none_or(|last| last.session.id != id) {
                result.push(report_row(row).map_err(storage_error)?);
            }
            let metric_id: Option<String> = row.get("metric_id").map_err(storage_error)?;
            if let (Some(metric_id), Some(last)) = (metric_id, result.last_mut()) {
                let value = row.get("value").map_err(storage_error)?;
                last.session.values.push(SessionValue { metric_id, value });
            }
        }
        Ok(result)
    }

    /// The live timestamped notes of each listed session, oldest first.
    pub fn session_notes_for(
        &self,
        session_ids: &[&str],
    ) -> Result<BTreeMap<String, Vec<SessionNote>>, Error> {
        let ids = serde_json::to_string(session_ids).map_err(|e| Error::Internal(e.to_string()))?;
        let notes = self.all(
            "SELECT session_id, id, at, text FROM session_notes
             WHERE deleted_at IS NULL AND session_id IN (SELECT value FROM json_each(?1))
             ORDER BY session_id, at, rowid",
            [ids],
            |row| {
                Ok((
                    row.get::<_, String>("session_id")?,
                    SessionNote {
                        id: row.get("id")?,
                        at: codec::get_timestamp(row, "at")?,
                        text: row.get("text")?,
                    },
                ))
            },
        )?;
        let mut by_session = BTreeMap::<String, Vec<SessionNote>>::new();
        for (session_id, note) in notes {
            by_session.entry(session_id).or_default().push(note);
        }
        Ok(by_session)
    }

    /// Each project's live tags by name.
    pub fn project_tags(&self) -> Result<BTreeMap<String, Vec<Tag>>, Error> {
        let links = self.all(
            "SELECT pt.project_id, t.* FROM project_tags pt JOIN tags t ON t.id = pt.tag_id
             WHERE t.deleted_at IS NULL ORDER BY pt.project_id, t.name, t.id",
            [],
            |row| Ok((row.get::<_, String>("project_id")?, codec::tag(row)?)),
        )?;
        let mut tags = BTreeMap::<String, Vec<Tag>>::new();
        for (project, tag) in links {
            tags.entry(project).or_default().push(tag);
        }
        Ok(tags)
    }
}

fn report_row(row: &rusqlite::Row) -> rusqlite::Result<ReportSession> {
    let session = codec::session(row)?;
    let project = match &session.project_id {
        Some(id) => Some(ProjectRef {
            id: id.clone(),
            name: row.get("project_name")?,
            color: row.get("project_color")?,
        }),
        None => None,
    };
    let item = match &session.item_id {
        Some(id) => Some(ItemRef {
            id: id.clone(),
            title: row.get("item_title")?,
        }),
        None => None,
    };
    Ok(ReportSession {
        session,
        project,
        epic_name: row.get("epic_name")?,
        item,
    })
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}
