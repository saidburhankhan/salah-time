//! A notes catalogue with one row per matching main note or timestamped entry.

use crate::{
    Store,
    sessions::notes::{encode_cursor, note_limit},
    stats::{Filter, Range, report_scope},
};
use chrono::{DateTime, NaiveDate, Utc};
use salah_domain::error::Error;
use salah_store::note_queries::{NoteCursor, NoteKind, NotesPage};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// Keep room in the 1 MiB frame for the cursor and JSON-RPC envelope.
const ROW_BYTES: usize = 768 * 1024;

#[derive(Deserialize)]
pub struct NoteQuery {
    pub q: Option<String>,
    #[serde(default)]
    pub kind: NoteKind,
    pub limit: Option<i64>,
    pub before: Option<String>,
}

#[derive(Serialize)]
pub struct NoteList {
    rows: Vec<NoteRow>,
    next: Option<String>,
    total: i64,
}

#[derive(Serialize)]
struct NoteRow {
    session_id: String,
    note_id: Option<String>,
    kind: &'static str,
    at: DateTime<Utc>,
    text: String,
    session: Context,
}

#[derive(Serialize)]
struct Context {
    day: NaiveDate,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
    minutes: Option<i64>,
    item_id: Option<String>,
    title: Option<String>,
    project_id: Option<String>,
    epic_id: Option<String>,
    values: BTreeMap<String, f64>,
    version: i64,
}

pub fn list(
    store: &Store,
    range: Range,
    filter: Filter,
    query: NoteQuery,
) -> Result<NoteList, Error> {
    let limit = note_limit(query.limit)?;
    let q = query.q.as_deref().unwrap_or("").trim().to_lowercase();
    let before: Option<NoteCursor> = query
        .before
        .as_deref()
        .map(|cursor| {
            serde_json::from_str::<NoteCursor>(cursor)
                .ok()
                .filter(|(_, id, _)| !id.is_empty())
                .ok_or_else(|| {
                    Error::validation("before", "before must be a previous page's next cursor")
                })
        })
        .transpose()?;
    store.read_snapshot(|store| {
        let scope = report_scope(store, &range, &filter)?;
        let (total, hits) = store.report_note_hits(
            &scope,
            NotesPage {
                kind: query.kind,
                query: &q,
                before: before.as_ref(),
                fetch: limit + 1,
            },
        )?;
        let ids = hits
            .iter()
            .take(limit)
            .map(|hit| hit.cursor.1.as_str())
            .collect::<BTreeSet<_>>();
        let sessions = store
            .report_sessions_by_ids(&ids.into_iter().collect::<Vec<_>>())?
            .into_iter()
            .map(|row| (row.session.id.clone(), row))
            .collect::<BTreeMap<_, _>>();
        let mut rows = Vec::new();
        let mut budget = ROW_BYTES;
        let mut last = None;
        for hit in hits.iter().take(limit) {
            let (at, id, note) = &hit.cursor;
            let report = &sessions[id];
            let session = &report.session;
            let row = NoteRow {
                session_id: id.clone(),
                note_id: (!note.is_empty()).then(|| note.clone()),
                at: *at,
                kind: if note.is_empty() { "main" } else { "entry" },
                text: hit.text.clone(),
                session: Context {
                    day: session.block.day,
                    start: session.started_at,
                    end: session.ended_at,
                    minutes: session.minutes,
                    item_id: session.item_id.clone(),
                    title: report.item.as_ref().map(|item| item.title.clone()),
                    project_id: session.project_id.clone(),
                    epic_id: session.epic_id.clone(),
                    values: session
                        .values
                        .iter()
                        .map(|v| (v.metric_id.clone(), v.value))
                        .collect(),
                    version: session.version,
                },
            };
            let bytes = serde_json::to_vec(&row)
                .map_err(|e| Error::Internal(e.to_string()))?
                .len()
                + 1;
            if bytes > budget {
                if rows.is_empty() {
                    return Err(Error::validation(
                        "limit",
                        "one note row exceeds the response size limit",
                    ));
                }
                break;
            }
            budget -= bytes;
            rows.push(row);
            last = Some(&hit.cursor);
        }
        let next = if rows.len() < hits.len() {
            last.map(encode_cursor).transpose()?
        } else {
            None
        };
        Ok(NoteList { rows, next, total })
    })
}
