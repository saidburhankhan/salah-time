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
    review::{Minutes, Reflection, validate_week},
    runtime::IdGenerator,
};
use salah_store::reports::{AssignmentGroup, CalendarGroup, SessionPage, SessionScope};
use serde::Serialize;

// Byte budgets include escaped JSON and array delimiters. Together they
// leave room in a 1 MiB frame for a full reflection, counts and the envelope.
const GROUP_BYTES: usize = 128 * 1024;
const NOTED_BYTES: usize = 384 * 1024;
const NOTED_SESSIONS: usize = 32;
const ENTRIES_PER_SESSION: u32 = 12;

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
    by_project_count: usize,
    by_epic_count: usize,
    goals_met_count: usize,
    sessions_with_notes: Vec<NotedSession>,
    sessions_with_notes_count: i64,
    sessions_with_notes_more: bool,
    open_backlog_count: i64,
    distractions_per_hour: Option<f64>,
    previous_distractions_per_hour: Option<f64>,
}
#[derive(Serialize)]
struct NotedSession {
    #[serde(flatten)]
    row: ListRow,
    notes_count: i64,
    notes_more: bool,
}

pub fn week(s: &str) -> Result<NaiveDate, Error> {
    let day = goals::parse_day("week", s)?;
    validate_week(day)?;
    Ok(day)
}

fn truncate_to_bytes<T: Serialize>(rows: &mut Vec<T>, budget: usize) -> Result<(), Error> {
    let mut size = 2;
    for (index, row) in rows.iter().enumerate() {
        size += serde_json::to_vec(row)
            .map_err(|e| Error::Internal(e.to_string()))?
            .len()
            + 1;
        if size > budget {
            rows.truncate(index);
            break;
        }
    }
    Ok(())
}

fn noted_sessions(store: &Store, scope: &SessionScope<'_>) -> Result<Vec<NotedSession>, Error> {
    let page = SessionPage {
        with_note: true,
        fetch: NOTED_SESSIONS,
        ..Default::default()
    };
    let mut preview = Vec::new();
    for row in store.report_sessions(scope, Some(&page))? {
        let (notes, notes_count) =
            store.review_note_entries(&row.session.id, ENTRIES_PER_SESSION)?;
        preview.push(NotedSession {
            notes_more: (notes.len() as i64) < notes_count,
            notes_count,
            row: ListRow::from_report(&row, notes),
        });
    }
    truncate_to_bytes(&mut preview, NOTED_BYTES)?;
    Ok(preview)
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
        let mut by_project = store.assignment_minutes(&scope, AssignmentGroup::Project)?;
        let mut by_epic = store.assignment_minutes(&scope, AssignmentGroup::Epic)?;
        let mut goals_met = goals::met_between(store, week, end.min(today))?;
        let by_project_count = by_project.len();
        let by_epic_count = by_epic.len();
        let goals_met_count = goals_met.len();
        truncate_to_bytes(&mut by_project, GROUP_BYTES)?;
        truncate_to_bytes(&mut by_epic, GROUP_BYTES)?;
        truncate_to_bytes(&mut goals_met, GROUP_BYTES)?;
        let sessions_with_notes_count = store.noted_session_count(&scope)?;
        let sessions_with_notes = noted_sessions(store, &scope)?;
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
                by_project,
                by_epic,
                goals_met,
                by_project_count,
                by_epic_count,
                goals_met_count,
                sessions_with_notes_more: (sessions_with_notes.len() as i64)
                    < sessions_with_notes_count,
                sessions_with_notes_count,
                sessions_with_notes,
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
