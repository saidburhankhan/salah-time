//! Weekly review. Only the user's reflection is persisted.
use crate::{
    Store, WriteTx, goals,
    sessions::ListRow,
    stats::{self, Filter, Range},
};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use salah_domain::{
    error::Error,
    goal::{GoalView, Period},
    review::{Reflection, validate_week},
    runtime::IdGenerator,
};
use salah_store::reports::CalendarGroup;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub struct Review {
    week: NaiveDate,
    summary: Summary,
    note: Option<String>,
    updated_at: Option<DateTime<Utc>>,
}
#[derive(Serialize)]
struct Summary {
    by_project: Vec<Minutes>,
    by_epic: Vec<Minutes>,
    goals_met: Vec<GoalView>,
    sessions_with_notes: Vec<ListRow>,
    open_backlog_count: i64,
    distractions_per_hour: Option<f64>,
    previous_distractions_per_hour: Option<f64>,
}
#[derive(Serialize)]
struct Minutes {
    id: String,
    name: String,
    minutes: i64,
}

pub fn week(s: &str) -> Result<NaiveDate, Error> {
    let day = goals::parse_day("week", s)?;
    validate_week(day)?;
    Ok(day)
}

fn minutes_rows(groups: BTreeMap<(String, String), i64>) -> Vec<Minutes> {
    groups
        .into_iter()
        .map(|((name, id), minutes)| Minutes { id, name, minutes })
        .collect()
}

pub fn get(store: &Store, week: NaiveDate, today: NaiveDate) -> Result<Review, Error> {
    validate_week(week)?;
    let end = Period::Week.bounds(week)?.1;
    store.read_snapshot(|store| {
        let filter = Filter::default();
        let mut scope = stats::report_scope(
            store,
            &Range {
                from: week,
                to: end,
            },
            &filter,
        )?;
        let mut projects = BTreeMap::new();
        let mut epics = BTreeMap::new();
        let mut rows = store.report_sessions(&scope, None)?;
        rows.sort_by(|a, b| {
            (b.session.block.day, b.sort_at(), &b.session.id).cmp(&(
                a.session.block.day,
                a.sort_at(),
                &a.session.id,
            ))
        });
        let ids: Vec<_> = rows.iter().map(|r| r.session.id.as_str()).collect();
        let mut notes = store.session_notes_for(&ids)?;
        let mut sessions_with_notes = Vec::new();
        for row in rows {
            if let Some(minutes) = row.session.minutes {
                let (id, name) = row
                    .project
                    .as_ref()
                    .map(|p| (p.id.as_str(), p.name.as_str()))
                    .unwrap_or((stats::NONE, "No project"));
                *projects
                    .entry((name.to_owned(), id.to_owned()))
                    .or_default() += minutes;
                let (id, name) = row
                    .session
                    .epic_id
                    .as_deref()
                    .zip(row.epic_name.as_deref())
                    .unwrap_or((stats::NONE, "No epic"));
                *epics.entry((name.to_owned(), id.to_owned())).or_default() += minutes;
            }
            let entries = notes.remove(&row.session.id).unwrap_or_default();
            if row.session.note.as_deref().is_some_and(|n| !n.is_empty()) || !entries.is_empty() {
                sessions_with_notes.push(ListRow::from_report(&row, entries));
            }
        }
        let previous = week - Duration::weeks(1);
        scope.from = previous;
        let rates = store.calendar_groups(&scope, CalendarGroup::Week)?;
        let rate = |day: NaiveDate| {
            rates
                .get(&day.to_string())
                .and_then(|m| m.distractions_per_hour)
        };
        let reflection = store.reflection(week)?;
        Ok(Review {
            week,
            updated_at: reflection.as_ref().map(|r| r.updated_at),
            note: reflection.map(|r| r.note),
            summary: Summary {
                by_project: minutes_rows(projects),
                by_epic: minutes_rows(epics),
                sessions_with_notes,
                goals_met: goals::met_between(store, week, end.min(today))?,
                open_backlog_count: store.open_backlog_count()?,
                distractions_per_hour: rate(week),
                previous_distractions_per_hour: rate(previous),
            },
        })
    })
}

/// Upsert one live reflection or soft-delete it when the text is blank.
pub fn save(
    tx: &WriteTx<'_>,
    week: NaiveDate,
    note: &str,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
) -> Result<Option<Reflection>, Error> {
    validate_week(week)?;
    let existing = tx.reflection(week)?;
    let blank = note.trim().is_empty();
    let mut row = match existing {
        Some(row) => row,
        None if blank => return Ok(None),
        None => Reflection {
            id: ids.new_id(),
            week,
            note: String::new(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            version: 1,
        },
    };
    if blank {
        row.deleted_at = Some(now);
    } else {
        row.note = note.to_owned();
    }
    row.updated_at = now;
    tx.put_reflection(&mut row)?;
    Ok(Some(row))
}
