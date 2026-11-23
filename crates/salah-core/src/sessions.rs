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
use std::collections::BTreeMap;

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
    /// An open timer observed strictly after its planning day's Sleep boundary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale: Option<bool>,
    pub note: Option<String>,
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
            return Ok(Self::for_day(session, Some(false)));
        }
        let today = day::current(config, now)?;
        let clock = day::clock_for_day(store, config, session.block.day, today)?;
        Ok(Self::for_day(session, Some(now > clock.sleep_at)))
    }

    pub(crate) fn for_day(session: &Session, past_sleep: Option<bool>) -> Self {
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
            stale: past_sleep.map(|past| session.is_running() && past),
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
    let ids: Vec<String> = match project_id {
        Some(id) => store
            .project_metrics(id)?
            .into_iter()
            .map(|link| link.metric_id)
            .collect(),
        None => vec!["focus".to_owned(), "quality".to_owned()],
    };
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
    let (item_id, project_id) = resolve(tx, target)?;
    if running(tx)?.is_some() {
        return Err(Error::Conflict("a session is already running".into()));
    }
    let block = schedule::resolve(config, now)?.current_block();
    let mut session = new_session(ids.new_id(), now, item_id, project_id, block);
    session.started_at = Some(now);
    session.version = tx.put_session(&session)?;
    Ok(session)
}

/// Stops the running session (`id`, or whichever runs) and records its
/// minutes and any supplied values or note.
/// A timer left running past its day ends at that day's sleep boundary.
pub fn stop(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    id: Option<&str>,
    values: Values,
    session_note: Option<String>,
) -> Result<Session, Error> {
    let session = running(tx)?
        .filter(|s| id.is_none_or(|id| s.id == id))
        .ok_or_else(|| not_found("running session", id.unwrap_or("")))?;
    finish(tx, config, now, session, values, session_note)
}

fn finish(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    mut session: Session,
    values: Values,
    session_note: Option<String>,
) -> Result<Session, Error> {
    let started = session.started_at.unwrap_or(now);
    let day = session.block.day;
    let today = day::current(config, now)?;
    day::freeze(tx, config, day, today)?;
    let day_end = day::clock_for_day(tx, config, day, today)?.sleep_at;
    let ended = now.min(day_end).max(started);
    let mut combined: Values = session
        .values
        .iter()
        .map(|v| (v.metric_id.clone(), v.value))
        .collect();
    combined.extend(values);
    session.values = checked_values(tx, combined, &session.values)?;
    if session_note.is_some() {
        session.note = note(session_note)?;
    }
    session.ended_at = Some(ended);
    session.minutes = Some(elapsed_minutes(started, ended));
    session.updated_at = now;
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
    session.values = checked_values(tx, new.values, &[])?;
    session.note = note(new.note)?;
    day::freeze(tx, config, block.day, schedule.day())?;
    let stopped = if session.is_tick() && session.item_id.is_some() {
        running(tx)?
            .filter(|timer| timer.item_id == session.item_id)
            .map(|timer| finish(tx, config, now, timer, Values::new(), None))
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

/// Whole minutes between two instants, rounded, from 1 to a day's 1440.
fn elapsed_minutes(start: DateTime<Utc>, end: DateTime<Utc>) -> i64 {
    ((end - start).num_seconds() + 30)
        .div_euclid(60)
        .clamp(1, 1440)
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
    fn a_timer_stops_with_rounded_minutes_and_optional_values() {
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

        let invalid =
            store.write(|tx| stop(tx, &config(), at(20, 13, 25), None, rated(6.0, 3.0), None));
        assert_eq!(invalid.unwrap_err().code(), "validation");

        let stopped = store
            .write(|tx| {
                stop(
                    tx,
                    &config(),
                    at(20, 13, 25),
                    Some(&started.id),
                    Values::from([("focus".into(), 4.0)]),
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
            .write(|tx| {
                edit(
                    tx,
                    &config(),
                    at(20, 14, 0),
                    &logged.id,
                    changes.clone(),
                    Some(1),
                )
            })
            .unwrap();
        assert_eq!(
            (edited.minutes, edited.block.start),
            (Some(40), BoundaryId::Asr)
        );
        let stale = store.write(|tx| remove(tx, &config(), at(20, 14, 0), &logged.id, Some(1)));
        assert_eq!(stale.unwrap_err().code(), "conflict");
        store
            .write(|tx| remove(tx, &config(), at(20, 14, 0), &logged.id, Some(2)))
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
        assert_eq!(
            ids,
            [
                ("focus", false),
                ("quality", false),
                ("distractions", false)
            ]
        );
        assert_eq!(prompt[0].scale_max, Some(5));
    }

    #[test]
    fn a_forgotten_timer_ends_at_its_day_sleep_boundary() {
        let mut store = Store::open_in_memory().unwrap();
        let quran = project(&mut store, "Quran");
        store
            .write(|tx| start(tx, &config(), at(20, 13, 0), ids(), on_project(&quran.id)))
            .unwrap();
        let stopped = store
            .write(|tx| stop(tx, &config(), at(23, 9, 0), None, rated(3.0, 3.0), None))
            .unwrap();
        // 13:00 to the 23:00 sleep boundary of the session's day.
        assert_eq!(stopped.minutes, Some(600));
        assert_eq!(stopped.ended_at, Some(at(20, 23, 0)));
    }

    #[test]
    fn early_sleep_ends_the_timer_and_the_planning_day_even_before_isha() {
        let mut store = Store::open_in_memory().unwrap();
        let project = project(&mut store, "Book");
        let config = Config {
            sleep_time: "14:00".into(),
            ..config()
        };
        store
            .write(|tx| start(tx, &config, at(20, 13, 0), ids(), on_project(&project.id)))
            .unwrap();
        let after = at(20, 21, 0);
        assert_eq!(day::current(&config, after).unwrap(), date(21));
        let stopped = store
            .write(|tx| stop(tx, &config, after, None, rated(3.0, 3.0), None))
            .unwrap();
        assert_eq!(stopped.ended_at, Some(at(20, 14, 0)));
        assert_eq!(stopped.minutes, Some(60));
        assert!(store.day_written(date(20)).unwrap());
        let day = day::day_plan(&mut store, &config, after, Some(date(20))).unwrap();
        assert_eq!(day.sleep.time, "14:00");
        let dhuhr = day
            .blocks
            .iter()
            .find(|b| b.start == BoundaryId::Dhuhr)
            .unwrap();
        let start = day
            .prayers
            .iter()
            .find(|p| p.id == BoundaryId::Dhuhr)
            .unwrap();
        assert_eq!(
            dhuhr.capacity_minutes,
            14 * 60 - salah_domain::day::parse_clock_minutes(&start.time).unwrap()
        );
        assert!(
            day.blocks
                .iter()
                .filter(|b| b.start > BoundaryId::Dhuhr)
                .all(|b| b.capacity_minutes == 0)
        );
    }

    #[test]
    fn archived_metrics_reject_new_samples_and_preserve_existing_samples() {
        let mut store = Store::open_in_memory().unwrap();
        let pages = store
            .write(|tx| {
                metrics::add(
                    tx,
                    at(20, 9, 0),
                    ids(),
                    metrics::NewMetric {
                        name: "Pages".into(),
                        unit: "pages".into(),
                        kind: MetricKind::Count,
                        scale_max: None,
                        rollup: None,
                    },
                )
            })
            .unwrap();
        let book = store
            .write(|tx| {
                projects::add(
                    tx,
                    at(20, 9, 0),
                    ids(),
                    projects::NewProject {
                        name: "Book".into(),
                        metrics: Some(vec![projects::MetricChoice {
                            metric_id: pages.id.clone(),
                            required: true,
                        }]),
                        ..projects::NewProject::default()
                    },
                )
            })
            .unwrap();
        let logged = store
            .write(|tx| {
                log(
                    tx,
                    &config(),
                    at(20, 13, 0),
                    ids(),
                    NewLog {
                        target: on_project(&book.id),
                        minutes: Some(10),
                        values: Values::from([(pages.id.clone(), 12.0)]),
                        ..NewLog::default()
                    },
                )
            })
            .unwrap();
        let archive = metrics::MetricChanges {
            archived: Some(true),
            ..metrics::MetricChanges::default()
        };
        store
            .write(|tx| metrics::edit(tx, at(20, 14, 0), &pages.id, archive.clone(), None))
            .unwrap();

        assert!(prompt(&store, Some(&book.id)).unwrap().is_empty());
        store
            .write(|tx| start(tx, &config(), at(20, 14, 0), ids(), on_project(&book.id)))
            .unwrap();
        let archived_sample = Values::from([(pages.id.clone(), 1.0)]);
        let invalid = store
            .write(|tx| {
                stop(
                    tx,
                    &config(),
                    at(20, 14, 30),
                    None,
                    archived_sample.clone(),
                    None,
                )
            })
            .unwrap_err();
        assert_eq!(invalid.code(), "validation");
        assert!(running(&store).unwrap().is_some());
        store
            .write(|tx| stop(tx, &config(), at(20, 14, 30), None, Values::new(), None))
            .expect("a stop without values succeeds");

        let keep = SessionChanges {
            values: Some(Values::from([(pages.id.clone(), 12.0)])),
            note: Some(Some("kept".into())),
            ..SessionChanges::default()
        };
        store
            .write(|tx| edit(tx, &config(), at(20, 15, 0), &logged.id, keep.clone(), None))
            .expect("an existing archived value stays valid");
        let visible_only = SessionChanges {
            values: Some(Values::new()),
            ..Default::default()
        };
        let edited = store
            .write(|tx| {
                edit(
                    tx,
                    &config(),
                    at(20, 15, 0),
                    &logged.id,
                    visible_only.clone(),
                    None,
                )
            })
            .unwrap();
        assert_eq!(edited.value(&pages.id), Some(12.0));
        let change = SessionChanges {
            values: Some(Values::from([(pages.id.clone(), 13.0)])),
            ..SessionChanges::default()
        };
        let rejected = store.write(|tx| {
            edit(
                tx,
                &config(),
                at(20, 15, 0),
                &logged.id,
                change.clone(),
                None,
            )
        });
        assert_eq!(rejected.unwrap_err().code(), "validation");
    }
}
