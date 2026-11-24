//! Bounded session previews and pages of full timestamped notes.
use super::{ListRow, find};
use crate::Store;
use chrono::{DateTime, Utc};
use salah_domain::{error::Error, tracking::SessionNote};
use salah_store::reports::ReportSession;
use serde::{Serialize, de::DeserializeOwned};

const ENTRIES_PER_PREVIEW: u32 = 12;
const DEFAULT_NOTE_PAGE: i64 = 50;
const MAX_NOTE_PAGE: i64 = 100;

#[derive(Serialize)]
pub struct NotePreview {
    #[serde(flatten)]
    pub row: ListRow,
    pub notes_count: i64,
    pub notes_more: bool,
}

impl NotePreview {
    pub(crate) fn new(store: &Store, row: &ReportSession) -> Result<Self, Error> {
        let (notes, count) = store.review_note_entries(&row.session.id, ENTRIES_PER_PREVIEW)?;
        Ok(Self::with_entries(row, notes, count))
    }

    pub(crate) fn with_entries(row: &ReportSession, notes: Vec<SessionNote>, count: i64) -> Self {
        Self {
            notes_more: (notes.len() as i64) < count,
            notes_count: count,
            row: ListRow::from_report(row, notes),
        }
    }
}

pub(crate) fn truncate_to_bytes<T: Serialize>(
    rows: &mut Vec<T>,
    budget: usize,
) -> Result<(), Error> {
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

pub(crate) fn note_limit(limit: Option<i64>) -> Result<usize, Error> {
    let limit = limit.unwrap_or(DEFAULT_NOTE_PAGE);
    if !(1..=MAX_NOTE_PAGE).contains(&limit) {
        return Err(Error::validation(
            "limit",
            format!("limit must be between 1 and {MAX_NOTE_PAGE}"),
        ));
    }
    Ok(limit as usize)
}

pub(crate) fn parse_after<T: DeserializeOwned>(after: Option<&str>) -> Result<Option<T>, Error> {
    after
        .map(|cursor| serde_json::from_str(cursor).map_err(|_| invalid_after()))
        .transpose()
}

pub(crate) fn invalid_after() -> Error {
    Error::validation("after", "after must be a previous page's next cursor")
}

pub(crate) fn encode_cursor(cursor: &impl Serialize) -> Result<String, Error> {
    serde_json::to_string(cursor).map_err(|e| Error::Internal(e.to_string()))
}

#[derive(Serialize)]
pub struct NotePage {
    notes: Vec<SessionNote>,
    next: Option<String>,
}

pub fn notes(
    store: &Store,
    id: &str,
    limit: Option<i64>,
    after: Option<&str>,
) -> Result<NotePage, Error> {
    let limit = note_limit(limit)?;
    let after: Option<(String, String, i64)> = parse_after(after)?;
    if after.as_ref().is_some_and(|(owner, at, rowid)| {
        owner != id || at.parse::<DateTime<Utc>>().is_err() || *rowid <= 0
    }) {
        return Err(invalid_after());
    }
    store.read_snapshot(|store| {
        find(store, id)?;
        let mut rows = store.session_note_page(
            id,
            after.as_ref().map(|(_, at, rowid)| (at.as_str(), *rowid)),
            limit + 1,
        )?;
        let next = if rows.len() > limit {
            rows.truncate(limit);
            rows.last()
                .map(|(_, at, rowid)| encode_cursor(&(id, at, rowid)))
                .transpose()?
        } else {
            None
        };
        Ok(NotePage {
            notes: rows.into_iter().map(|(note, _, _)| note).collect(),
            next,
        })
    })
}
