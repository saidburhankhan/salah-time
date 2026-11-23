//! Sessions: time spent on an item or project, from a timer or logged by
//! hand, with the metric values recorded when it ended.

mod list;
mod running;
pub use list::{ListRow, SessionList, SessionQuery, ValueRange, list};
pub use running::{Stop, StopResult, append_note, bump, stop};

use crate::config::Config;
use crate::guard::{ensure_version, minutes, not_found, note, nullable};
use crate::{Store, WriteTx, day, items, metrics, projects};
use chrono::{DateTime, NaiveDate, Utc};
use salah_domain::{
    day::{BlockKey, BoundaryId},
    error::Error,
    item::validate_block,
    runtime::IdGenerator,
    tracking::{MetricKind, Session, SessionNote, SessionValue},
};
use salah_prayer::schedule;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What a session is for: an item (its project is copied), a project, or both.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Target {
    #[serde(default)]
    pub item_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    pub epic_id: Option<String>,
}

/// Metric values keyed by metric id, as clients send them.
pub type Values = BTreeMap<String, f64>;

/// A finished session recorded without a timer: minutes, or a tick without.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct NewLog {
    #[serde(flatten)]
    pub target: Target,
    /// Defaults to the day in progress.
    #[serde(default)]
    pub date: Option<NaiveDate>,
    /// Defaults to the item's block on another `date`, else the block in progress.
    #[serde(default)]
    pub block: Option<BoundaryId>,
    #[serde(default)]
    pub minutes: Option<i64>,
    #[serde(default)]
    pub values: Values,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SessionChanges {
    #[serde(default)]
    pub minutes: Option<i64>,
    #[serde(default)]
    pub values: Option<Values>,
    /// `null` clears the note; an absent field keeps it.
    #[serde(default, deserialize_with = "nullable")]
    pub note: Option<Option<String>>,
    #[serde(default)]
    pub block: Option<BoundaryId>,
    #[serde(default)]
    pub remove_note_ids: Vec<String>,
}

/// A session as clients see it: values keyed by metric id.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SessionView {
    pub id: String,
    pub item_id: Option<String>,
    pub project_id: Option<String>,
    pub epic_id: Option<String>,
    pub day: NaiveDate,
    pub block: BoundaryId,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    /// `null` while running and for a tick.
    pub minutes: Option<i64>,
    pub running: bool,
    /// An open timer observed strictly after its planning day's Sleep boundary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale: Option<bool>,
    /// The most minutes `session.stop` accepts right now (a running timer
    /// only): elapsed time, cut off at its planning day's Sleep boundary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_minutes: Option<i64>,
    pub note: Option<String>,
    pub notes: Vec<SessionNote>,
    pub values: Values,
    pub version: i64,
}

impl SessionView {
    pub fn at(
        store: &Store,
        config: &Config,
        now: DateTime<Utc>,
        session: &Session,
    ) -> Result<Self, Error> {
        if !session.is_running() {
            return Self::for_day(store, session, Some(false));
        }
        let today = day::current(config, now)?;
        let clock = day::clock_for_day(store, config, session.block.day, today)?;
        Self::timed(store, session, now, clock.sleep_at)
    }

    /// The view at `now` given the Sleep boundary of the session's day.
    pub(crate) fn timed(
        store: &Store,
        session: &Session,
        now: DateTime<Utc>,
        sleep_at: DateTime<Utc>,
    ) -> Result<Self, Error> {
        let mut view = Self::for_day(store, session, Some(now > sleep_at))?;
        if session.is_running() {
            view.max_minutes = Some(running::max_minutes(session, now, sleep_at));
        }
        Ok(view)
    }

    pub(crate) fn for_day(
        store: &Store,
        session: &Session,
        past_sleep: Option<bool>,
    ) -> Result<Self, Error> {
        Ok(Self::with_notes(
            session,
            store.session_notes(&session.id)?,
            past_sleep,
        ))
    }

    pub(crate) fn with_notes(
        session: &Session,
        notes: Vec<SessionNote>,
        past_sleep: Option<bool>,
    ) -> Self {
        Self {
            id: session.id.clone(),
            item_id: session.item_id.clone(),
            project_id: session.project_id.clone(),
            epic_id: session.epic_id.clone(),
            day: session.block.day,
            block: session.block.start,
            started_at: session.started_at,
            ended_at: session.ended_at,
            minutes: session.minutes,
            running: session.is_running(),
            stale: past_sleep.map(|past| session.is_running() && past),
            max_minutes: None,
            note: session.note.clone(),
            notes,
            values: session
                .values
                .iter()
                .map(|v| (v.metric_id.clone(), v.value))
                .collect(),
            version: session.version,
        }
    }
}

/// An optional metric available for the running session.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PromptMetric {
    pub metric_id: String,
    pub name: String,
    pub unit: String,
    pub kind: MetricKind,
    pub scale_max: Option<i64>,
    pub required: bool,
}

/// The running session, if any.
pub fn running(store: &Store) -> Result<Option<Session>, Error> {
    Ok(store.running_sessions()?.into_iter().next())
}

/// The metrics a session of `project_id` asks for, in the project's order.
pub fn prompt(store: &Store, project_id: Option<&str>) -> Result<Vec<PromptMetric>, Error> {
    selected_prompt(store, project_id, None)
}

pub(crate) fn selected_prompt(
    store: &Store,
    project_id: Option<&str>,
    selection: Option<&[String]>,
) -> Result<Vec<PromptMetric>, Error> {
    if selection.is_some_and(<[String]>::is_empty) {
        return Ok(Vec::new());
    }
    let mut ids: Vec<String> = match project_id {
        Some(id) => store
            .project_metrics(id)?
            .into_iter()
            .map(|link| link.metric_id)
            .collect(),
        None => ["focus", "quality", "distractions"]
            .map(str::to_owned)
            .into(),
    };
    if let Some(selected) = selection {
        ids.retain(|id| selected.contains(id));
        for id in selected {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
    }
    let mut prompt = Vec::new();
    for id in ids {
        let metric = metrics::find(store, &id)?;
        if !metric.archived {
            prompt.push(PromptMetric {
                metric_id: metric.id,
                name: metric.name,
                unit: metric.unit,
                kind: metric.kind,
                scale_max: metric.scale_max,
                required: false,
            });
        }
    }
    Ok(prompt)
}

/// Starts a timer in the block in progress. Only one session may run.
pub fn start(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    target: Target,
) -> Result<Session, Error> {
    let target = resolve(tx, target)?;
    if running(tx)?.is_some() {
        return Err(Error::Conflict("a session is already running".into()));
    }
    let block = schedule::resolve(config, now)?.current_block();
    let mut session = new_session(ids.new_id(), now, target, block);
    session.started_at = Some(now);
    session.version = tx.put_session(&session)?;
    Ok(session)
}

/// Records a finished session with optional metric values.
pub fn log(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    new: NewLog,
) -> Result<Session, Error> {
    Ok(log_with_changes(tx, config, now, ids, new)?.logged)
}

/// The new log and any timer stopped in the same transaction.
pub struct LogResult {
    pub logged: Session,
    pub stopped: Option<Session>,
}

pub fn log_with_changes(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    new: NewLog,
) -> Result<LogResult, Error> {
    let item_block = match &new.target.item_id {
        Some(id) => Some(items::find(tx, id)?.block),
        None => None,
    };
    let target = resolve(tx, new.target)?;
    let schedule = schedule::resolve(config, now)?;
    let current = schedule.current_block();
    let start = match (new.block, new.date, item_block) {
        (Some(block), _, _) => block,
        (None, Some(_), Some(block)) => block,
        _ => current.start,
    };
    let block = BlockKey {
        day: new.date.unwrap_or(current.day),
        start: validate_block(start)?,
    };
    let mut session = new_session(ids.new_id(), now, target, block);
    session.minutes = new.minutes.map(|m| minutes("minutes", m)).transpose()?;
    session.values = checked_values(tx, new.values, &[])?;
    session.note = note(new.note)?;
    day::freeze(tx, config, block.day, schedule.day())?;
    let stopped = if session.is_tick() && session.item_id.is_some() {
        running(tx)?
            .filter(|timer| timer.item_id == session.item_id)
            .map(|timer| running::finish(tx, config, now, timer, Stop::default()))
            .transpose()?
    } else {
        None
    };
    session.version = tx.put_session(&session)?;
    Ok(LogResult {
        logged: session,
        stopped,
    })
}

pub fn edit(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    changes: SessionChanges,
    expected_version: Option<i64>,
) -> Result<Session, Error> {
    let mut session = find(tx, id)?;
    ensure_version("session", id, session.version, expected_version)?;
    day::freeze(tx, config, session.block.day, day::current(config, now)?)?;
    if let Some(value) = changes.minutes {
        if session.is_running() {
            return Err(Error::validation(
                "minutes",
                "stop the session before changing its minutes",
            ));
        }
        session.minutes = Some(minutes("minutes", value)?);
    }
    if let Some(values) = changes.values {
        session.values = checked_values(tx, values, &session.values)?;
    }
    if let Some(value) = changes.note {
        session.note = note(value)?;
    }
    if let Some(block) = changes.block {
        session.block.start = validate_block(block)?;
    }
    tx.remove_session_notes(id, &changes.remove_note_ids, now)?;
    session.updated_at = now;
    session.version = tx.put_session(&session)?;
    Ok(session)
}

pub fn remove(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    expected_version: Option<i64>,
) -> Result<Session, Error> {
    let mut session = find(tx, id)?;
    ensure_version("session", id, session.version, expected_version)?;
    day::freeze(tx, config, session.block.day, day::current(config, now)?)?;
    tx.remove_all_session_notes(id, now)?;
    session.deleted_at = Some(now);
    session.updated_at = now;
    session.version = tx.put_session(&session)?;
    Ok(session)
}

pub fn find(store: &Store, id: &str) -> Result<Session, Error> {
    store
        .session(id)?
        .filter(Session::is_live)
        .ok_or_else(|| not_found("session", id))
}

fn new_session(id: String, now: DateTime<Utc>, target: Target, block: BlockKey) -> Session {
    Session {
        id,
        item_id: target.item_id,
        project_id: target.project_id,
        epic_id: target.epic_id,
        block,
        started_at: None,
        ended_at: None,
        minutes: None,
        note: None,
        values: Vec::new(),
        created_at: now,
        updated_at: now,
        deleted_at: None,
        version: 1,
    }
}

/// Resolve ownership once when the session is created.
fn resolve(store: &Store, mut target: Target) -> Result<Target, Error> {
    if let Some(id) = &target.item_id {
        let item = items::find(store, id)?;
        if target.project_id.is_some() && target.project_id != item.project_id {
            return Err(Error::validation(
                "project_id",
                "project_id does not match the item's project",
            ));
        }
        if target.epic_id.is_some() && target.epic_id != item.epic_id {
            return Err(Error::validation(
                "epic_id",
                "epic_id does not match the item's epic",
            ));
        }
        target.project_id = item.project_id;
        target.epic_id = item.epic_id;
    } else {
        if let Some(id) = &target.epic_id {
            let epic = crate::epics::find(store, id)?;
            if target
                .project_id
                .as_ref()
                .is_some_and(|p| *p != epic.project_id)
            {
                return Err(Error::validation(
                    "project_id",
                    "project_id does not match the epic's project",
                ));
            }
            target.project_id = Some(epic.project_id);
        }
        let project = target.project_id.as_deref().ok_or_else(|| {
            Error::validation(
                "item_id",
                "a session needs an item_id, epic_id or project_id",
            )
        })?;
        projects::active(store, project)?;
    }
    Ok(target)
}

/// Validates supplied samples and preserves previously recorded archived values.
fn checked_values(
    store: &Store,
    mut values: Values,
    kept: &[SessionValue],
) -> Result<Vec<SessionValue>, Error> {
    for (metric_id, value) in &values {
        let metric = metrics::find(store, metric_id)?;
        let unchanged = kept
            .iter()
            .any(|v| v.metric_id == *metric_id && v.value == *value);
        if !(metric.archived && unchanged) {
            metric
                .validate_value(*value)
                .map_err(|message| Error::validation("values", message))?;
        }
    }
    for old in kept {
        if !values.contains_key(&old.metric_id) && metrics::find(store, &old.metric_id)?.archived {
            values.insert(old.metric_id.clone(), old.value);
        }
    }
    Ok(values
        .into_iter()
        .map(|(metric_id, value)| SessionValue { metric_id, value })
        .collect())
}

#[cfg(test)]
mod tests;
