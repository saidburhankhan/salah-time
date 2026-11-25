//! Versioned changes to timestamped session entries.

use super::{find, running::require_live_timer};
use crate::{
    WriteTx,
    config::Config,
    day,
    guard::{ensure_version, note_text},
};
use chrono::{DateTime, Duration, FixedOffset, Utc};
use salah_domain::{
    day::parse_clock_minutes,
    error::Error,
    runtime::IdGenerator,
    tracking::{Session, SessionNote},
};

#[derive(Default)]
pub struct NewNote<'a> {
    pub session_id: Option<&'a str>,
    pub text: &'a str,
    pub at: Option<&'a str>,
    pub expected_version: Option<i64>,
}

pub fn append_note(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    draft: NewNote<'_>,
) -> Result<Session, Error> {
    let text = note_text("text", draft.text)?;
    let mut session = if let (Some(id), Some(_)) = (draft.session_id, draft.at) {
        let session = find(tx, id)?;
        if session.is_running() {
            require_live_timer(tx, config, now, Some(id))?
        } else {
            session
        }
    } else {
        require_live_timer(tx, config, now, draft.session_id)?
    };
    ensure_version(
        "session",
        &session.id,
        session.version,
        draft.expected_version,
    )?;
    let at = draft
        .at
        .map(|time| note_instant(tx, config, now, &session, time))
        .transpose()?
        .unwrap_or(now);
    let entry = SessionNote {
        id: ids.new_id(),
        at,
        text,
    };
    session.version = tx.insert_session_note(&session.id, &entry, now)?;
    session.updated_at = now;
    Ok(session)
}

fn note_instant(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    session: &Session,
    time: &str,
) -> Result<DateTime<Utc>, Error> {
    let minutes = parse_clock_minutes(time).map_err(|e| Error::validation("at", e.to_string()))?;
    let today = day::current(config, now)?;
    let day = session.block.day;
    let clock = day::clock_for_day(tx, config, day, today)?;
    let invalid = || Error::validation("at", "time must fall within the session");
    let (start, end) = if let Some(start) = session.started_at {
        (start, session.ended_at.unwrap_or(now))
    } else {
        let block = day::report_blocks(tx, config, day, today)?
            .into_iter()
            .find(|b| b.block == session.block.start)
            .ok_or_else(invalid)?;
        (block.starts_at, block.ends_at)
    };
    let zone = FixedOffset::east_opt(clock.offset_seconds)
        .ok_or_else(|| Error::Storage("invalid snapshot timezone offset".into()))?;
    let dates = start.with_timezone(&zone).date_naive().iter_days();
    dates
        .take_while(|date| *date <= end.with_timezone(&zone).date_naive())
        .find_map(|date| {
            let midnight = date
                .and_hms_opt(0, 0, 0)?
                .and_local_timezone(zone)
                .single()?;
            let selected = (midnight + Duration::minutes(minutes)).to_utc();
            // A minute picked at the start may contain the timer's start seconds.
            let within_end = if session.started_at.is_some() {
                selected <= end
            } else {
                selected < end
            };
            (within_end && selected + Duration::minutes(1) > start).then(|| selected.max(start))
        })
        .ok_or_else(invalid)
}

pub fn edit_note(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    session_id: &str,
    note_id: &str,
    text: &str,
    expected_version: i64,
) -> Result<Session, Error> {
    let text = note_text("text", text)?;
    change_note(tx, now, session_id, note_id, Some(&text), expected_version)
}

pub fn remove_note(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    session_id: &str,
    note_id: &str,
    expected_version: i64,
) -> Result<Session, Error> {
    change_note(tx, now, session_id, note_id, None, expected_version)
}

fn change_note(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    session_id: &str,
    note_id: &str,
    text: Option<&str>,
    expected_version: i64,
) -> Result<Session, Error> {
    let mut session = find(tx, session_id)?;
    ensure_version(
        "session",
        session_id,
        session.version,
        Some(expected_version),
    )?;
    session.version = tx.change_session_note(session_id, note_id, text, now)?;
    session.updated_at = now;
    Ok(session)
}
