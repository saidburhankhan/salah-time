//! Tracking-model use cases. Protocol adapters are deliberately kept outside this module.
use crate::config::Config;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use salah_domain::{
    day::{BlockKey, BoundaryId},
    error::Error,
    item::validate_block,
    runtime::IdGenerator,
    tracking::*,
};
use salah_prayer::schedule;
use salah_store::Store;
use serde::Serialize;
use std::collections::BTreeSet;

fn live_item(store: &Store, id: &str) -> Result<Item, Error> {
    store
        .item(id)?
        .filter(Item::is_live)
        .ok_or_else(|| Error::NotFound {
            what: "item",
            id: id.into(),
        })
}
fn live_project(store: &Store, id: &str) -> Result<Project, Error> {
    store
        .project(id)?
        .filter(|p| p.deleted_at.is_none() && !p.archived)
        .ok_or_else(|| Error::NotFound {
            what: "project",
            id: id.into(),
        })
}
fn resolve_project(
    store: &Store,
    item_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<(Option<String>, Option<String>), Error> {
    if let Some(id) = item_id {
        let item = live_item(store, id)?;
        if let Some(given) = project_id
            && item.project_id.as_deref() != Some(given)
        {
            return Err(Error::validation(
                "project_id",
                "project does not match item",
            ));
        }
        Ok((Some(item.id), item.project_id))
    } else if let Some(id) = project_id {
        Ok((None, Some(live_project(store, id)?.id)))
    } else {
        Err(Error::validation(
            "item_id",
            "item_id or project_id is required",
        ))
    }
}
fn validate_values(
    store: &Store,
    project: Option<&str>,
    values: &[SessionValue],
    required: bool,
) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for v in values {
        if !seen.insert(v.metric_id.as_str()) {
            return Err(Error::validation("values", "duplicate metric"));
        }
        let m = store.metric(&v.metric_id)?.ok_or_else(|| Error::NotFound {
            what: "metric",
            id: v.metric_id.clone(),
        })?;
        m.validate_value(v.value)
            .map_err(|e| Error::validation("values", e))?;
    }
    if required && let Some(id) = project {
        let missing: Vec<_> = store
            .project_metrics(id)?
            .into_iter()
            .filter(|m| m.required && !seen.contains(m.metric_id.as_str()))
            .map(|m| m.metric_id)
            .collect();
        if !missing.is_empty() {
            return Err(Error::validation(
                "values",
                format!("missing required metrics: {}", missing.join(",")),
            ));
        }
    }
    Ok(())
}
fn rounded_minutes(start: DateTime<Utc>, end: DateTime<Utc>) -> i64 {
    ((end - start).num_seconds() + 30).div_euclid(60).max(1)
}

pub fn session_start(
    store: &mut Store,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    item_id: Option<&str>,
    project_id: Option<&str>,
    block: BlockKey,
) -> Result<Session, Error> {
    let (i, p) = resolve_project(store, item_id, project_id)?;
    if store
        .running_sessions()?
        .into_iter()
        .any(|s| s.is_running())
    {
        return Err(Error::Conflict("a session is already running".into()));
    }
    let mut s = Session {
        id: ids.new_id(),
        item_id: i,
        project_id: p,
        block,
        started_at: Some(now),
        ended_at: None,
        minutes: None,
        note: None,
        values: vec![],
        created_at: now,
        updated_at: now,
        deleted_at: None,
        version: 1,
    };
    s.version = store.write(|tx| tx.put_session(&s))?;
    Ok(s)
}
/// A completed session recorded without a timer (manual minutes or a tick).
#[derive(Clone, Debug, Default)]
pub struct SessionLog {
    pub item_id: Option<String>,
    pub project_id: Option<String>,
    pub minutes: Option<i64>,
    pub values: Vec<SessionValue>,
    pub note: Option<String>,
}

pub fn session_log(
    store: &mut Store,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    block: BlockKey,
    log: SessionLog,
) -> Result<Session, Error> {
    let SessionLog {
        item_id,
        project_id,
        minutes,
        values,
        note,
    } = log;
    if minutes.is_some_and(|m| m < 1) {
        return Err(Error::validation("minutes", "minutes must be positive"));
    }
    validate_block(block.start)?;
    let (i, p) = resolve_project(store, item_id.as_deref(), project_id.as_deref())?;
    validate_values(store, p.as_deref(), &values, true)?;
    let mut s = Session {
        id: ids.new_id(),
        item_id: i,
        project_id: p,
        block,
        started_at: None,
        ended_at: None,
        minutes,
        note,
        values,
        created_at: now,
        updated_at: now,
        deleted_at: None,
        version: 1,
    };
    s.version = store.write(|tx| tx.put_session(&s))?;
    Ok(s)
}
pub fn session_stop(
    store: &mut Store,
    now: DateTime<Utc>,
    id: Option<&str>,
    values: Vec<SessionValue>,
    note: Option<String>,
) -> Result<Session, Error> {
    let mut running = store.running_sessions()?;
    let mut s = if let Some(id) = id {
        running.into_iter().find(|s| s.id == id)
    } else if running.len() == 1 {
        running.pop()
    } else {
        None
    }
    .ok_or_else(|| Error::NotFound {
        what: "running session",
        id: id.unwrap_or("").into(),
    })?;
    validate_values(store, s.project_id.as_deref(), &values, true)?;
    let start = s
        .started_at
        .ok_or_else(|| Error::Conflict("session is not running".into()))?;
    s.ended_at = Some(now);
    s.minutes = Some(rounded_minutes(start, now));
    s.values = values;
    s.note = note;
    s.updated_at = now;
    s.version = store.write(|tx| tx.put_session(&s))?;
    Ok(s)
}
pub fn session_edit(
    store: &mut Store,
    now: DateTime<Utc>,
    id: &str,
    minutes: Option<i64>,
    values: Option<Vec<SessionValue>>,
    note: Option<Option<String>>,
    block: Option<BoundaryId>,
) -> Result<Session, Error> {
    let mut s = store
        .session(id)?
        .filter(Session::is_live)
        .ok_or_else(|| Error::NotFound {
            what: "session",
            id: id.into(),
        })?;
    if let Some(m) = minutes {
        if m < 1 {
            return Err(Error::validation("minutes", "minutes must be positive"));
        }
        s.minutes = Some(m)
    }
    if let Some(v) = values {
        validate_values(store, s.project_id.as_deref(), &v, true)?;
        s.values = v
    }
    if let Some(n) = note {
        s.note = n
    }
    if let Some(b) = block {
        validate_block(b)?;
        s.block.start = b
    }
    s.updated_at = now;
    s.version = store.write(|tx| tx.put_session(&s))?;
    Ok(s)
}
pub fn session_remove(store: &mut Store, now: DateTime<Utc>, id: &str) -> Result<Session, Error> {
    let mut s = store
        .session(id)?
        .filter(Session::is_live)
        .ok_or_else(|| Error::NotFound {
            what: "session",
            id: id.into(),
        })?;
    s.deleted_at = Some(now);
    s.updated_at = now;
    s.version = store.write(|tx| tx.put_session(&s))?;
    Ok(s)
}

pub fn materialize_day(
    store: &mut Store,
    config: &Config,
    day: NaiveDate,
    today: NaiveDate,
) -> Result<(Vec<DayBlock>, Vec<DayPlanLine>), Error> {
    let mut blocks = store.day_blocks(day)?;
    if blocks.is_empty() {
        blocks = schedule::day_blocks(config, day)?
            .into_iter()
            .map(|w| DayBlock {
                day,
                block: w.key.start,
                starts_at: w.start,
                ends_at: w.end,
                source: DayBlockSource::Calculated,
            })
            .collect();
        store.write(|tx| tx.replace_day_blocks(day, &blocks))?
    }
    let mut plan = store.day_plan(day)?;
    if plan.is_empty() || day == today {
        plan = store
            .items()?
            .into_iter()
            .filter(|i| i.applies_to(day))
            .map(|i| DayPlanLine {
                day,
                block: i.block,
                item_id: i.id,
                planned_minutes: i.planned_minutes,
            })
            .collect();
        store.write(|tx| tx.replace_day_plan(day, &plan))?
    }
    Ok((blocks, plan))
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Fulfilment {
    pub planned_items: i64,
    pub done_items: i64,
    pub planned_minutes: i64,
    pub actual_minutes: i64,
}
pub fn fulfilment(store: &Store, day: NaiveDate) -> Result<Fulfilment, Error> {
    let plan = store.day_plan(day)?;
    let sessions: Vec<_> = store
        .sessions_between(day, day)?
        .into_iter()
        .filter(Session::is_live)
        .collect();
    let done = plan
        .iter()
        .filter(|p| {
            sessions
                .iter()
                .any(|s| s.item_id.as_deref() == Some(&p.item_id))
        })
        .count() as i64;
    Ok(Fulfilment {
        planned_items: plan.len() as i64,
        done_items: done,
        planned_minutes: plan.iter().filter_map(|p| p.planned_minutes).sum(),
        actual_minutes: sessions.iter().filter_map(|s| s.minutes).sum(),
    })
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct DayStat {
    pub day: NaiveDate,
    pub minutes: i64,
    pub capacity_minutes: i64,
    pub planned_minutes: i64,
    pub fulfilment_ratio: f64,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MetricPoint {
    pub day: NaiveDate,
    pub value: Option<f64>,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MetricSeries {
    pub metric_id: String,
    pub name: String,
    pub unit: String,
    pub rollup: Rollup,
    pub points: Vec<MetricPoint>,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Stats {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub days: Vec<DayStat>,
    pub series: Vec<MetricSeries>,
    pub totals: StatsTotals,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct StatsTotals {
    pub minutes: i64,
    pub sessions: i64,
    pub active_days: i64,
    pub avg_focus: Option<f64>,
    pub avg_quality: Option<f64>,
}
pub fn stats(
    store: &Store,
    from: NaiveDate,
    to: NaiveDate,
    project: Option<&str>,
) -> Result<Stats, Error> {
    let sessions: Vec<_> = store
        .sessions_between(from, to)?
        .into_iter()
        .filter(|s| s.is_live() && project.is_none_or(|p| s.project_id.as_deref() == Some(p)))
        .collect();
    let mut days = vec![];
    let mut day = from;
    while day <= to {
        let ss: Vec<_> = sessions.iter().filter(|s| s.block.day == day).collect();
        let mins = ss.iter().filter_map(|s| s.minutes).sum();
        let blocks = store.day_blocks(day)?;
        let capacity = blocks
            .iter()
            .map(|b| (b.ends_at - b.starts_at).num_minutes().max(0))
            .sum();
        let planned = store
            .day_plan(day)?
            .iter()
            .filter_map(|p| p.planned_minutes)
            .sum();
        days.push(DayStat {
            day,
            minutes: mins,
            capacity_minutes: capacity,
            planned_minutes: planned,
            fulfilment_ratio: if planned > 0 {
                mins as f64 / planned as f64
            } else {
                0.0
            },
        });
        day += Duration::days(1)
    }
    let metrics = store.metrics()?;
    let mut series = vec![];
    for m in metrics {
        let mut points = vec![];
        for d in &days {
            let vals: Vec<_> = sessions
                .iter()
                .filter(|s| s.block.day == d.day)
                .flat_map(|s| {
                    s.values
                        .iter()
                        .filter(|v| v.metric_id == m.id)
                        .map(move |v| (v.value, s.minutes.unwrap_or(1).max(1) as f64))
                })
                .collect();
            let value = if vals.is_empty() {
                None
            } else if m.rollup == Rollup::Sum {
                Some(vals.iter().map(|v| v.0).sum())
            } else {
                let weight: f64 = vals.iter().map(|v| v.1).sum();
                Some(vals.iter().map(|v| v.0 * v.1).sum::<f64>() / weight)
            };
            points.push(MetricPoint { day: d.day, value })
        }
        series.push(MetricSeries {
            metric_id: m.id,
            name: m.name,
            unit: m.unit,
            rollup: m.rollup,
            points,
        })
    }
    let avg = |id: &str| {
        let mut sum = 0.;
        let mut weight = 0.;
        for s in &sessions {
            for v in &s.values {
                if v.metric_id == id {
                    let w = s.minutes.unwrap_or(1).max(1) as f64;
                    sum += v.value * w;
                    weight += w
                }
            }
        }
        if weight > 0. {
            Some(sum / weight)
        } else {
            None
        }
    };
    Ok(Stats {
        from,
        to,
        totals: StatsTotals {
            minutes: sessions.iter().filter_map(|s| s.minutes).sum(),
            sessions: sessions.len() as i64,
            active_days: days.iter().filter(|d| d.minutes > 0).count() as i64,
            avg_focus: avg("focus"),
            avg_quality: avg("quality"),
        },
        days,
        series,
    })
}
