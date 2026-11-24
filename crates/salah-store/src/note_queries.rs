use crate::{
    Store,
    codec::{self, storage_error},
    reports::SORT_AT,
};
use chrono::NaiveDate;
use rusqlite::params;
use salah_domain::{error::Error, tracking::SessionNote};

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
