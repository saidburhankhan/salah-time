//! Sessions: time spent on an item or project, from a timer or logged by
//! hand, with the metric values recorded when it ended.

use crate::config::Config;
use crate::guard::{ensure_version, minutes, not_found, note, nullable};
use crate::{Store, WriteTx, day, items, metrics, projects};
use chrono::{DateTime, NaiveDate, Utc};
use salah_domain::{
    day::{BlockKey, BoundaryId},
    error::Error,
    item::validate_block,
    runtime::IdGenerator,
    tracking::{MetricKind, Session, SessionValue},
};
use salah_prayer::schedule;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// What a session is for: an item (its project is copied), a project, or both.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Target {
    #[serde(default)]
    pub item_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
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
}

/// A session as clients see it: values keyed by metric id.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SessionView {
    pub id: String,
    pub item_id: Option<String>,
    pub project_id: Option<String>,
    pub day: NaiveDate,
    pub block: BoundaryId,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    /// `null` while running and for a tick.
    pub minutes: Option<i64>,
    pub running: bool,
    pub note: Option<String>,
    pub values: Values,
    pub version: i64,
}

impl From<&Session> for SessionView {
    fn from(session: &Session) -> Self {
        Self {
            id: session.id.clone(),
            item_id: session.item_id.clone(),
            project_id: session.project_id.clone(),
            day: session.block.day,
            block: session.block.start,
            started_at: session.started_at,
            ended_at: session.ended_at,
            minutes: session.minutes,
            running: session.is_running(),
            note: session.note.clone(),
            values: session
                .values
                .iter()
                .map(|v| (v.metric_id.clone(), v.value))
                .collect(),
            version: session.version,
        }
    }
}

/// A metric to ask for when the running session stops.
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
    let Some(project_id) = project_id else {
        return Ok(Vec::new());
    };
    store
        .project_metrics(project_id)?
        .into_iter()
        .map(|link| {
            let metric = metrics::find(store, &link.metric_id)?;
            Ok(PromptMetric {
                metric_id: metric.id,
                name: metric.name,
                unit: metric.unit,
                kind: metric.kind,
                scale_max: metric.scale_max,
                required: link.required,
            })
        })
        .collect()
}

/// Starts a timer in the block in progress. Only one session may run.
pub fn start(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    target: Target,
) -> Result<Session, Error> {
    let (item_id, project_id) = resolve(tx, target)?;
    if running(tx)?.is_some() {
        return Err(Error::Conflict("a session is already running".into()));
    }
    let schedule = schedule::resolve(config, now)?;
    let block = schedule.current_block();
    day::materialize(tx, config, block.day, schedule.day())?;
    let mut session = new_session(ids.new_id(), now, item_id, project_id, block);
    session.started_at = Some(now);
    session.version = tx.put_session(&session)?;
    Ok(session)
}

/// Stops the running session (`id`, or whichever runs) and records its
/// minutes, values, and note. The project's required metrics must be given.
pub fn stop(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    id: Option<&str>,
    values: Values,
    session_note: Option<String>,
) -> Result<Session, Error> {
    let mut session = running(tx)?
        .filter(|s| id.is_none_or(|id| s.id == id))
        .ok_or_else(|| not_found("running session", id.unwrap_or("")))?;
    let started = session.started_at.unwrap_or(now);
    session.values = checked_values(tx, session.project_id.as_deref(), values)?;
    session.note = note(session_note)?;
    session.ended_at = Some(now.max(started));
    session.minutes = Some(elapsed_minutes(started, now));
    session.updated_at = now;
    session.version = tx.put_session(&session)?;
    Ok(session)
}

/// Records a finished session. The project's required metrics must be given.
pub fn log(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    new: NewLog,
) -> Result<Session, Error> {
    let item_block = match &new.target.item_id {
        Some(id) => Some(items::find(tx, id)?.block),
        None => None,
    };
    let (item_id, project_id) = resolve(tx, new.target)?;
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
    let mut session = new_session(ids.new_id(), now, item_id, project_id, block);
    session.minutes = new.minutes.map(|m| minutes("minutes", m)).transpose()?;
    session.values = checked_values(tx, session.project_id.as_deref(), new.values)?;
    session.note = note(new.note)?;
    day::materialize(tx, config, block.day, schedule.day())?;
    session.version = tx.put_session(&session)?;
    Ok(session)
}

pub fn edit(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    id: &str,
    changes: SessionChanges,
    expected_version: Option<i64>,
) -> Result<Session, Error> {
    let mut session = find(tx, id)?;
    ensure_version("session", id, session.version, expected_version)?;
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
        session.values = checked_values(tx, session.project_id.as_deref(), values)?;
    }
    if let Some(value) = changes.note {
        session.note = note(value)?;
    }
    if let Some(block) = changes.block {
        session.block.start = validate_block(block)?;
    }
    session.updated_at = now;
    session.version = tx.put_session(&session)?;
    Ok(session)
}

pub fn remove(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    id: &str,
    expected_version: Option<i64>,
) -> Result<Session, Error> {
    let mut session = find(tx, id)?;
    ensure_version("session", id, session.version, expected_version)?;
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

/// Whole minutes between two instants, rounded, at least 1.
fn elapsed_minutes(start: DateTime<Utc>, end: DateTime<Utc>) -> i64 {
    ((end - start).num_seconds() + 30).div_euclid(60).max(1)
}

fn new_session(
    id: String,
    now: DateTime<Utc>,
    item_id: Option<String>,
    project_id: Option<String>,
    block: BlockKey,
) -> Session {
    Session {
        id,
        item_id,
        project_id,
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

/// The item and project a session records; the item's project wins.
fn resolve(store: &Store, target: Target) -> Result<(Option<String>, Option<String>), Error> {
    match (target.item_id, target.project_id) {
        (Some(item_id), given) => {
            let item = items::find(store, &item_id)?;
            if given.is_some() && given != item.project_id {
                return Err(Error::validation(
                    "project_id",
                    "project_id does not match the item's project",
                ));
            }
            Ok((Some(item.id), item.project_id))
        }
        (None, Some(project_id)) => Ok((None, Some(projects::active(store, &project_id)?.id))),
        (None, None) => Err(Error::validation(
            "item_id",
            "a session needs an item_id or a project_id",
        )),
    }
}

/// Validates each value against its metric and requires the project's
/// required metrics.
fn checked_values(
    store: &Store,
    project_id: Option<&str>,
    values: Values,
) -> Result<Vec<SessionValue>, Error> {
    for (metric_id, value) in &values {
        metrics::find(store, metric_id)?
            .validate_value(*value)
            .map_err(|message| Error::validation("values", message))?;
    }
    let given: BTreeSet<&str> = values.keys().map(String::as_str).collect();
    let missing: Vec<String> = match project_id {
        Some(id) => store
            .project_metrics(id)?
            .into_iter()
            .filter(|link| link.required && !given.contains(link.metric_id.as_str()))
            .map(|link| link.metric_id)
            .collect(),
        None => Vec::new(),
    };
    if !missing.is_empty() {
        return Err(Error::MissingMetrics(missing));
    }
    Ok(values
        .into_iter()
        .map(|(metric_id, value)| SessionValue { metric_id, value })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{at, config, date, ids, project};

    fn rated(focus: f64, quality: f64) -> Values {
        Values::from([("focus".into(), focus), ("quality".into(), quality)])
    }

    fn on_project(id: &str) -> Target {
        Target {
            item_id: None,
            project_id: Some(id.into()),
        }
    }

    #[test]
    fn a_timer_stops_with_rounded_minutes_and_required_values() {
        let mut store = Store::open_in_memory().unwrap();
        let quran = project(&mut store, "Quran");
        let started = store
            .write(|tx| start(tx, &config(), at(20, 13, 0), ids(), on_project(&quran.id)))
            .unwrap();
        assert!(started.is_running());
        assert_eq!(started.block.start, BoundaryId::Dhuhr);
        let second =
            store.write(|tx| start(tx, &config(), at(20, 13, 1), ids(), on_project(&quran.id)));
        assert_eq!(second.unwrap_err().code(), "conflict");

        let missing = store.write(|tx| {
            stop(
                tx,
                at(20, 13, 25),
                None,
                Values::from([("focus".into(), 4.0)]),
                None,
            )
        });
        assert_eq!(
            missing.unwrap_err(),
            Error::MissingMetrics(vec!["quality".into()])
        );
        let invalid = store.write(|tx| stop(tx, at(20, 13, 25), None, rated(6.0, 3.0), None));
        assert_eq!(invalid.unwrap_err().code(), "validation");

        let stopped = store
            .write(|tx| {
                stop(
                    tx,
                    at(20, 13, 25),
                    Some(&started.id),
                    rated(4.0, 3.0),
                    Some(" good ".into()),
                )
            })
            .unwrap();
        assert_eq!(stopped.minutes, Some(25));
        assert_eq!(stopped.note.as_deref(), Some("good"));
        assert_eq!(running(&store).unwrap(), None);
    }

    #[test]
    fn a_log_without_minutes_is_a_tick_and_copies_the_item_project() {
        let mut store = Store::open_in_memory().unwrap();
        let quran = project(&mut store, "Quran");
        let item = store
            .write(|tx| {
                items::add(
                    tx,
                    &config(),
                    at(20, 6, 0),
                    ids(),
                    items::NewItem {
                        title: "Read".into(),
                        block: Some(BoundaryId::Fajr),
                        project_id: Some(quran.id.clone()),
                        ..items::NewItem::default()
                    },
                )
            })
            .unwrap();
        let tick = store
            .write(|tx| {
                log(
                    tx,
                    &config(),
                    at(20, 13, 0),
                    ids(),
                    NewLog {
                        target: Target {
                            item_id: Some(item.id.clone()),
                            project_id: None,
                        },
                        date: Some(date(19)),
                        values: rated(5.0, 5.0),
                        ..NewLog::default()
                    },
                )
            })
            .unwrap();
        assert_eq!(tick.minutes, None);
        assert_eq!(tick.project_id.as_deref(), Some(quran.id.as_str()));
        assert_eq!(
            tick.block,
            BlockKey {
                day: date(19),
                start: BoundaryId::Fajr
            },
            "a dated log lands in the item's own block"
        );
        let without_target =
            store.write(|tx| log(tx, &config(), at(20, 13, 0), ids(), NewLog::default()));
        assert_eq!(without_target.unwrap_err().code(), "validation");
    }

    #[test]
    fn edit_and_remove_respect_versions() {
        let mut store = Store::open_in_memory().unwrap();
        let quran = project(&mut store, "Quran");
        let logged = store
            .write(|tx| {
                log(
                    tx,
                    &config(),
                    at(20, 13, 0),
                    ids(),
                    NewLog {
                        target: on_project(&quran.id),
                        minutes: Some(10),
                        values: rated(3.0, 3.0),
                        ..NewLog::default()
                    },
                )
            })
            .unwrap();
        let changes = SessionChanges {
            minutes: Some(40),
            block: Some(BoundaryId::Asr),
            ..SessionChanges::default()
        };
        let edited = store
            .write(|tx| edit(tx, at(20, 14, 0), &logged.id, changes.clone(), Some(1)))
            .unwrap();
        assert_eq!(
            (edited.minutes, edited.block.start),
            (Some(40), BoundaryId::Asr)
        );
        let stale = store.write(|tx| remove(tx, at(20, 14, 0), &logged.id, Some(1)));
        assert_eq!(stale.unwrap_err().code(), "conflict");
        store
            .write(|tx| remove(tx, at(20, 14, 0), &logged.id, Some(2)))
            .unwrap();
        assert_eq!(find(&store, &logged.id).unwrap_err().code(), "not_found");
    }

    #[test]
    fn prompt_lists_the_project_metrics_in_order() {
        let mut store = Store::open_in_memory().unwrap();
        let quran = project(&mut store, "Quran");
        let prompt = prompt(&store, Some(&quran.id)).unwrap();
        let ids: Vec<_> = prompt
            .iter()
            .map(|m| (m.metric_id.as_str(), m.required))
            .collect();
        assert_eq!(ids, [("focus", true), ("quality", true)]);
        assert_eq!(prompt[0].scale_max, Some(5));
    }
}
