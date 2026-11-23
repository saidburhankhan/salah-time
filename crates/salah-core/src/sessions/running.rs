//! Changes made to a timer while it is running or as it stops.

use super::{Values, checked_values, new_session, running};
use crate::config::Config;
use crate::guard::{label, not_found, note};
use crate::{Store, WriteTx, day, metrics};
use chrono::{DateTime, Utc};
use salah_domain::{
    error::Error,
    runtime::IdGenerator,
    tracking::{MetricKind, Session, SessionNote, SessionValue},
};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Stop {
    #[serde(default)]
    pub values: Values,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub minutes: Option<i64>,
    #[serde(default)]
    pub done: bool,
}

pub struct StopResult {
    pub stopped: Session,
    pub tick: Option<Session>,
}

/// Records the timer and, for Save & Done, its item's completion on the timer's day.
pub fn stop(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    id: Option<&str>,
    options: Stop,
) -> Result<StopResult, Error> {
    let timer = require_running(tx, id)?;
    let tick_item = if options.done {
        let item_id = timer
            .item_id
            .as_deref()
            .ok_or_else(|| Error::validation("done", "done requires an item timer"))?;
        Some(
            tx.item(item_id)?
                .ok_or_else(|| not_found("item", item_id))?,
        )
    } else {
        None
    };
    let stopped = finish(tx, config, now, timer, options)?;
    let tick = tick_item
        .map(|mut item| -> Result<Session, Error> {
            let day = stopped.block.day;
            if let Some(tick) = tx
                .sessions_between_for_item(day, day, Some(&item.id))?
                .into_iter()
                .find(Session::is_tick)
            {
                return Ok(tick);
            }
            crate::epics::detach_removed(tx, &mut item)?;
            let mut tick = new_session(
                ids.new_id(),
                now,
                super::Target {
                    item_id: Some(item.id),
                    project_id: item.project_id,
                    epic_id: item.epic_id,
                },
                stopped.block,
            );
            tick.version = tx.put_session(&tick)?;
            Ok(tick)
        })
        .transpose()?;
    Ok(StopResult { stopped, tick })
}

fn require_running(store: &Store, id: Option<&str>) -> Result<Session, Error> {
    running(store)?
        .filter(|s| id.is_none_or(|id| s.id == id))
        .ok_or_else(|| not_found("running session", id.unwrap_or("")))
}

/// The running timer, refused once it ran past its planning day's Sleep
/// boundary: a count or note at `now` would land on the earlier day's
/// session after the instant it will be cut off at.
fn require_live_timer(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    id: Option<&str>,
) -> Result<Session, Error> {
    let session = require_running(tx, id)?;
    let today = day::current(config, now)?;
    if now > day::clock_for_day(tx, config, session.block.day, today)?.sleep_at {
        return Err(Error::validation(
            "id",
            "the timer ran past Sleep; stop it first",
        ));
    }
    Ok(session)
}

/// Adjusts one active count metric on the running timer.
pub fn bump(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    id: Option<&str>,
    metric_id: &str,
    delta: i64,
) -> Result<Session, Error> {
    if !matches!(delta, -1 | 1) {
        return Err(Error::validation("delta", "delta must be 1 or -1"));
    }
    let mut session = require_live_timer(tx, config, now, id)?;
    let metric = metrics::find(tx, metric_id)?;
    if metric.kind != MetricKind::Count || metric.archived {
        return Err(Error::validation(
            "metric_id",
            "bump requires an active count metric",
        ));
    }
    let value = (session.value(metric_id).unwrap_or(0.0) + delta as f64).max(0.0);
    session.version = tx.put_session_value(&session.id, metric_id, value, now)?;
    session.updated_at = now;
    if let Some(sample) = session
        .values
        .iter_mut()
        .find(|sample| sample.metric_id == metric_id)
    {
        sample.value = value;
    } else {
        session.values.push(SessionValue {
            metric_id: metric_id.to_owned(),
            value,
        });
    }
    Ok(session)
}

/// Appends one timestamped entry to the running timer.
pub fn append_note(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    id: Option<&str>,
    text: &str,
) -> Result<Session, Error> {
    let text = label("text", text, 500)?;
    let mut session = require_live_timer(tx, config, now, id)?;
    let entry = SessionNote {
        id: ids.new_id(),
        at: now,
        text,
    };
    session.version = tx.add_session_note(&session.id, &entry)?;
    session.updated_at = now;
    Ok(session)
}

pub(super) fn finish(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    mut session: Session,
    options: Stop,
) -> Result<Session, Error> {
    let started = session.started_at.unwrap_or(now);
    let day = session.block.day;
    let today = day::current(config, now)?;
    day::freeze(tx, config, day, today)?;
    let day_end = day::clock_for_day(tx, config, day, today)?.sleep_at;
    let ended = cutoff(started, now, day_end);
    let elapsed = elapsed_minutes(started, ended);
    let worked = options.minutes.unwrap_or(elapsed);
    if !(1..=elapsed).contains(&worked) {
        return Err(Error::validation(
            "minutes",
            format!("minutes must be between 1 and {elapsed}"),
        ));
    }
    let mut combined: Values = session
        .values
        .iter()
        .map(|v| (v.metric_id.clone(), v.value))
        .collect();
    combined.extend(options.values);
    session.values = checked_values(tx, combined, &session.values)?;
    if options.note.is_some() {
        session.note = note(options.note)?;
    }
    session.ended_at = Some(ended);
    session.minutes = Some(worked);
    session.updated_at = now;
    session.version = tx.put_session(&session)?;
    Ok(session)
}

/// Where a timer stopped at `now` ends: never past its day's Sleep boundary.
fn cutoff(started: DateTime<Utc>, now: DateTime<Utc>, sleep_at: DateTime<Utc>) -> DateTime<Utc> {
    now.min(sleep_at).max(started)
}

/// The upper bound of `session.stop.minutes` for `timer` at `now`.
pub(super) fn max_minutes(timer: &Session, now: DateTime<Utc>, sleep_at: DateTime<Utc>) -> i64 {
    let started = timer.started_at.unwrap_or(now);
    elapsed_minutes(started, cutoff(started, now, sleep_at))
}

/// Whole minutes between two instants, rounded, from 1 to a day's 1440.
fn elapsed_minutes(start: DateTime<Utc>, end: DateTime<Utc>) -> i64 {
    ((end - start).num_seconds() + 30)
        .div_euclid(60)
        .clamp(1, 1440)
}
