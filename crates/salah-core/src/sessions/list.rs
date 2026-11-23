//! `sessions.list`: finished sessions newest first, for the Notes page. Every
//! predicate, the cursor and the limit run in SQL.

use super::SessionView;
use crate::stats::{Filter, Range, report_scope};
use crate::{Store, metrics};
use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use salah_domain::{
    error::Error,
    tracking::{ItemRef, ProjectRef},
};
use salah_store::reports::{Bounds, SessionPage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_LIMIT: i64 = 100;
pub const MAX_LIMIT: i64 = 500;

/// The Notes page's predicates beyond the shared range and filter.
#[derive(Clone, Debug, Deserialize)]
pub struct SessionQuery {
    /// Only sessions with a main note or a timestamped entry (default true).
    #[serde(default = "yes")]
    pub with_note: bool,
    /// Case-insensitive text in the main note or any entry.
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub minutes_min: Option<i64>,
    #[serde(default)]
    pub minutes_max: Option<i64>,
    /// Inclusive bounds per metric; a session lacking the value does not match.
    #[serde(default)]
    pub values: BTreeMap<String, ValueRange>,
    #[serde(default)]
    pub limit: Option<i64>,
    /// The previous page's `next`.
    #[serde(default)]
    pub before: Option<String>,
}

impl Default for SessionQuery {
    fn default() -> Self {
        Self {
            with_note: true,
            q: None,
            minutes_min: None,
            minutes_max: None,
            values: BTreeMap::new(),
            limit: None,
            before: None,
        }
    }
}

fn yes() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub struct ValueRange {
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SessionList {
    pub rows: Vec<ListRow>,
    /// The cursor of the next page (`before`), `null` on the last one.
    pub next: Option<String>,
}

/// A session with the current labels of its recorded project and task.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ListRow {
    #[serde(flatten)]
    pub session: SessionView,
    pub project: Option<ProjectRef>,
    pub item: Option<ItemRef>,
}

/// One page of finished sessions (ticks included) in scope: newest planning
/// day first, then newest end instant (creation for manual logs and ticks),
/// then id.
pub fn list(
    store: &Store,
    range: Range,
    filter: Filter,
    query: SessionQuery,
) -> Result<SessionList, Error> {
    let scope = report_scope(store, &range, &filter)?;
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(Error::validation(
            "limit",
            format!("limit must be between 1 and {MAX_LIMIT}"),
        ));
    }
    for (field, minutes) in [
        ("minutes_min", query.minutes_min),
        ("minutes_max", query.minutes_max),
    ] {
        if minutes.is_some_and(|m| m < 0) {
            return Err(Error::validation(field, "minutes cannot be negative"));
        }
    }
    if let (Some(min), Some(max)) = (query.minutes_min, query.minutes_max)
        && min > max
    {
        return Err(Error::validation(
            "minutes_max",
            "minutes_max is below minutes_min",
        ));
    }
    let mut values = BTreeMap::new();
    for (metric_id, range) in &query.values {
        metrics::find(store, metric_id)?;
        if let (Some(min), Some(max)) = (range.min, range.max)
            && min > max
        {
            return Err(Error::validation(
                "values",
                format!("the {metric_id} range ends below its start"),
            ));
        }
        values.insert(
            metric_id.as_str(),
            Bounds {
                min: range.min,
                max: range.max,
            },
        );
    }
    let q = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(str::to_lowercase);
    let before = query.before.as_deref().map(parse_cursor).transpose()?;
    let page = SessionPage {
        with_note: query.with_note,
        q: q.as_deref(),
        minutes_min: query.minutes_min,
        minutes_max: query.minutes_max,
        values,
        before,
        fetch: limit as usize + 1,
    };
    let (mut sessions, mut notes) = store.read_snapshot(|store| {
        let sessions = store.report_sessions(&scope, Some(&page))?;
        let ids: Vec<&str> = sessions.iter().map(|s| s.session.id.as_str()).collect();
        let notes = store.session_notes_for(&ids)?;
        Ok((sessions, notes))
    })?;
    let next = if sessions.len() > limit as usize {
        sessions.truncate(limit as usize);
        sessions
            .last()
            .map(|s| cursor(s.session.block.day, s.sort_at(), &s.session.id))
    } else {
        None
    };
    let rows = sessions
        .into_iter()
        .map(|s| ListRow {
            session: SessionView::with_notes(
                &s.session,
                notes.remove(&s.session.id).unwrap_or_default(),
                Some(false),
            ),
            project: s.project,
            item: s.item,
        })
        .collect();
    Ok(SessionList { rows, next })
}

/// `<day>|<UTC instant>|<id>`, the ordering key of the last row on a page.
fn cursor(day: NaiveDate, at: DateTime<Utc>, id: &str) -> String {
    format!(
        "{day}|{}|{id}",
        at.to_rfc3339_opts(SecondsFormat::Millis, true)
    )
}

fn parse_cursor(text: &str) -> Result<(NaiveDate, DateTime<Utc>, &str), Error> {
    let invalid = || Error::validation("before", "before must be a previous page's next cursor");
    let mut parts = text.splitn(3, '|');
    let (Some(day), Some(at), Some(id)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(invalid());
    };
    let day = NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| invalid())?;
    let at = DateTime::parse_from_rfc3339(at).map_err(|_| invalid())?;
    if id.is_empty() {
        return Err(invalid());
    }
    Ok((day, at.with_timezone(&Utc), id))
}

#[cfg(test)]
mod tests;
