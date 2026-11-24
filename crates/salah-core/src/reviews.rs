//! Weekly review. Only the user's reflection is persisted.
use crate::{
    Store, WriteTx, goals,
    sessions::notes::{
        NotePreview, encode_cursor, invalid_after, note_limit, parse_after, truncate_to_bytes,
    },
    stats::{self, Filter, Range},
};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use salah_domain::{
    error::Error,
    goal::{GoalView, Period},
    review::{Minutes, Reflection, validate_week},
    runtime::IdGenerator,
};
use salah_store::note_queries::ReviewNoteCursor;
use salah_store::reports::{AssignmentGroup, CalendarGroup, SessionPage, SessionScope};
use serde::Serialize;
use std::collections::BTreeMap;

// Byte budgets include escaped JSON and array delimiters. Together they
// leave room in a 1 MiB frame for a full reflection, counts and the envelope.
const GROUP_BYTES: usize = 128 * 1024;
const NOTED_BYTES: usize = 384 * 1024;
const NOTED_SESSIONS: usize = 32;

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
    sessions_with_notes: Vec<NotePreview>,
    sessions_with_notes_count: i64,
    sessions_with_notes_more: bool,
    open_backlog_count: i64,
    distractions_per_hour: Option<f64>,
    previous_distractions_per_hour: Option<f64>,
}
pub fn week(s: &str) -> Result<NaiveDate, Error> {
    let day = goals::parse_day("week", s)?;
    validate_week(day)?;
    Ok(day)
}

fn noted_sessions(store: &Store, scope: &SessionScope<'_>) -> Result<Vec<NotePreview>, Error> {
    let page = SessionPage {
        with_note: true,
        fetch: NOTED_SESSIONS,
        ..Default::default()
    };
    let mut preview = store
        .report_sessions(scope, Some(&page))?
        .iter()
        .map(|row| NotePreview::new(store, row))
        .collect::<Result<Vec<_>, _>>()?;
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

#[derive(Serialize)]
pub struct ReviewNotes {
    rows: Vec<ReviewNoteRow>,
    next: Option<String>,
}

#[derive(Serialize)]
struct ReviewNoteRow {
    #[serde(flatten)]
    session: NotePreview,
    main_note_matches: bool,
}

pub fn notes(
    store: &Store,
    week: NaiveDate,
    query: &str,
    limit: Option<i64>,
    after: Option<&str>,
) -> Result<ReviewNotes, Error> {
    validate_week(week)?;
    let end = Period::Week.bounds(week)?.1;
    let limit = note_limit(limit)?;
    let query = query.trim().to_lowercase();
    let after: Option<ReviewNoteCursor> = parse_after(after)?;
    if after.as_ref().is_some_and(|cursor| {
        !(week..=end).contains(&cursor.0)
            || cursor.1.parse::<DateTime<Utc>>().is_err()
            || cursor.2.is_empty()
    }) {
        return Err(invalid_after());
    }
    store.read_snapshot(|store| {
        let hits = store.review_note_hits(week, end, &query, after.as_ref(), limit + 1)?;
        let page = &hits[..hits.len().min(limit)];
        let ids: Vec<_> = page.iter().map(|hit| hit.cursor.2.as_str()).collect();
        let sessions: BTreeMap<_, _> = store
            .report_sessions_by_ids(&ids)?
            .into_iter()
            .map(|session| (session.session.id.clone(), session))
            .collect();
        let mut rows = Vec::new();
        let mut cursors = Vec::new();
        for group in page.chunk_by(|a, b| a.cursor.2 == b.cursor.2) {
            let id = &group[0].cursor.2;
            let entries = group
                .iter()
                .rev()
                .filter_map(|hit| hit.entry.clone())
                .collect();
            rows.push(ReviewNoteRow {
                session: NotePreview::with_entries(
                    &sessions[id],
                    entries,
                    store.session_note_count(id)?,
                ),
                main_note_matches: group.iter().any(|hit| hit.entry.is_none()),
            });
            cursors.push(&group.last().expect("non-empty hit group").cursor);
        }
        let matched_rows = rows.len();
        truncate_to_bytes(&mut rows, NOTED_BYTES)?;
        if matched_rows > 0 && rows.is_empty() {
            return Err(Error::Internal(
                "review notes row exceeds the response budget".into(),
            ));
        }
        let more = hits.len() > limit || rows.len() < matched_rows;
        let next = if more {
            Some(encode_cursor(cursors[rows.len() - 1])?)
        } else {
            None
        };
        Ok(ReviewNotes { rows, next })
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
