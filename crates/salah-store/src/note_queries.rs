use crate::{
    Store,
    codec::{self, storage_error},
    reports::{Query, SORT_AT, SessionScope},
};
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, params_from_iter};
use salah_domain::{error::Error, tracking::SessionNote};
use serde::Deserialize;

/// The ordering key for a review note hit. Empty entry keys identify the main note.
pub type ReviewNoteCursor = (NaiveDate, String, String, String, String);

pub struct ReviewNoteHit {
    pub cursor: ReviewNoteCursor,
    pub entry: Option<SessionNote>,
}

impl Store {
    pub fn session_note_page(
        &self,
        id: &str,
        after: Option<(&str, i64)>,
        fetch: usize,
    ) -> Result<Vec<(SessionNote, String, i64)>, Error> {
        let (at, rowid) = after.unzip();
        self.all(
            "SELECT id, at, text, rowid FROM session_notes
             WHERE session_id=?1 AND deleted_at IS NULL AND (?2 IS NULL OR (at,rowid)>(?2,?3))
             ORDER BY at,rowid LIMIT ?4",
            params![id, at, rowid, fetch as i64],
            |row| {
                Ok((
                    SessionNote {
                        id: row.get("id")?,
                        at: codec::get_timestamp(row, "at")?,
                        text: row.get("text")?,
                    },
                    row.get("at")?,
                    row.get("rowid")?,
                ))
            },
        )
    }

    pub fn session_note_count(&self, id: &str) -> Result<i64, Error> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM session_notes WHERE session_id=?1 AND deleted_at IS NULL",
                [id],
                |r| r.get(0),
            )
            .map_err(storage_error)
    }

    pub fn review_note_hits(
        &self,
        week: NaiveDate,
        end: NaiveDate,
        query: &str,
        after: Option<&ReviewNoteCursor>,
        fetch: usize,
    ) -> Result<Vec<ReviewNoteHit>, Error> {
        let sql = format!(
            "WITH finished AS MATERIALIZED (
                SELECT s.day, {SORT_AT} AS sort_at, s.id, s.note FROM sessions s
                WHERE s.day BETWEEN ?1 AND ?2 AND s.deleted_at IS NULL
                  AND (s.started_at IS NULL OR s.ended_at IS NOT NULL)
             ), hits(day, sort_at, session_id, entry_at, entry_id, body) AS (
                SELECT day, sort_at, id, '', '', note FROM finished
                WHERE length(note)>0 AND instr(salah_lower(note),?3)>0
                UNION ALL
                SELECT s.day, s.sort_at, s.id, n.at, n.id, n.text FROM finished s
                CROSS JOIN session_notes n ON n.session_id=s.id
                WHERE n.deleted_at IS NULL AND instr(salah_lower(n.text),?3)>0
             )
             SELECT * FROM hits
             WHERE ?4 IS NULL OR (day,sort_at,session_id,entry_at,entry_id)<(?4,?5,?6,?7,?8)
             ORDER BY day DESC, sort_at DESC, session_id DESC, entry_at DESC, entry_id DESC
             LIMIT ?9"
        );
        self.all(
            &sql,
            params![
                codec::day(week),
                codec::day(end),
                query,
                after.map(|c| codec::day(c.0)),
                after.map(|c| &c.1),
                after.map(|c| &c.2),
                after.map(|c| &c.3),
                after.map(|c| &c.4),
                fetch as i64
            ],
            |row| {
                let entry_id: String = row.get("entry_id")?;
                let entry = if entry_id.is_empty() {
                    None
                } else {
                    Some(SessionNote {
                        id: entry_id.clone(),
                        at: codec::get_timestamp(row, "entry_at")?,
                        text: row.get("body")?,
                    })
                };
                Ok(ReviewNoteHit {
                    cursor: (
                        codec::get_day(row, "day")?,
                        row.get("sort_at")?,
                        row.get("session_id")?,
                        row.get("entry_at")?,
                        entry_id,
                    ),
                    entry,
                })
            },
        )
    }
}

/// Newest first by note instant, session id, then note id (empty for a main note).
pub type NoteCursor = (DateTime<Utc>, String, String);

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    #[default]
    All,
    Main,
    Entry,
}

pub struct NoteHit {
    pub cursor: NoteCursor,
    pub text: String,
}

pub struct NotesPage<'a> {
    pub kind: NoteKind,
    pub query: &'a str,
    pub before: Option<&'a NoteCursor>,
    pub fetch: usize,
}

impl Store {
    pub fn report_note_hits(
        &self,
        scope: &SessionScope<'_>,
        page: NotesPage<'_>,
    ) -> Result<(i64, Vec<NoteHit>), Error> {
        let Query { sql, mut args } = Query::scoped(scope);
        let main = format!(
            "SELECT {SORT_AT} AS at, s.id AS session_id, '' AS note_id, s.note AS body
             FROM matched s WHERE length(s.note)>0"
        );
        let entries = "SELECT strftime('%Y-%m-%dT%H:%M:%fZ',n.at) AS at, s.id AS session_id,
                              n.id AS note_id, n.text AS body
                       FROM matched s JOIN session_notes n ON n.session_id=s.id
                       WHERE n.deleted_at IS NULL";
        let notes = match page.kind {
            NoteKind::Main => main,
            NoteKind::Entry => entries.to_owned(),
            NoteKind::All => format!("{main} UNION ALL {entries}"),
        };
        let mut select = format!("WITH matched AS MATERIALIZED ({sql}), notes AS ({notes})");
        let mut predicate = "WHERE 1".to_owned();
        if !page.query.is_empty() {
            predicate.push_str(" AND instr(salah_lower(body), ?)>0");
            args.push(page.query.to_owned().into());
        }
        let total = self
            .conn
            .query_row(
                &format!("{select} SELECT count(*) FROM notes {predicate}"),
                params_from_iter(&args),
                |r| r.get(0),
            )
            .map_err(storage_error)?;
        if let Some((at, session, note)) = page.before {
            predicate.push_str(" AND (at,session_id,note_id) < (?,?,?)");
            args.extend([
                codec::timestamp(*at).into(),
                session.clone().into(),
                note.clone().into(),
            ]);
        }
        select.push_str(&format!(
            " SELECT * FROM notes {predicate}
            ORDER BY at DESC, session_id DESC, note_id DESC LIMIT ?"
        ));
        args.push((page.fetch as i64).into());
        let hits = self.all(&select, params_from_iter(args), |r| {
            Ok(NoteHit {
                cursor: (
                    codec::get_timestamp(r, "at")?,
                    r.get("session_id")?,
                    r.get("note_id")?,
                ),
                text: r.get("body")?,
            })
        })?;
        Ok((total, hits))
    }
}
