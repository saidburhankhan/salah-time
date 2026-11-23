//! `stats.table`: the general statistics table. One row per week (Monday
//! first), day, project, tag or task, with minutes, sessions and every metric
//! rolled up like `stats.get` (an `avg` weighted by session minutes, a tick
//! weighing 1; a `sum` added up). Only finished sessions count.
//!
//! Unassigned work is a row of its own (`key: "none"`), so the rows of a
//! project, tag or task table always cover every session in scope. A session
//! counts once in each tag of its project; totals never double count.

use super::{Calendar, Filter, NONE, Range, aggregate, blocks, report_scope};
use crate::Store;

use chrono::{Datelike, Days, NaiveDate};
use salah_domain::{
    error::Error,
    tracking::{Metric, MetricKind, ProjectMetric, Rollup, Session, Tag},
};
use salah_store::reports::{Choice, ReportSession};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The built-in count with a derived rate column.
pub const DISTRACTIONS: &str = "distractions";
/// The derived column: distractions over the hours of the row's sessions.
pub const DISTRACTIONS_PER_HOUR: &str = "distractions_per_hour";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    #[default]
    Week,
    Day,
    Block,
    Project,
    Epic,
    Tag,
    Item,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Table {
    pub range: Range,
    pub filter: Filter,
    pub group: Group,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    pub totals: Totals,
}

/// `minutes`, `sessions`, then each metric; a derived column follows its source.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Column {
    pub id: String,
    pub name: String,
    pub unit: String,
    pub kind: MetricKind,
    pub rollup: Rollup,
    pub scale_max: Option<i64>,
    pub archived: bool,
    /// The metric a computed column derives from; `null` for stored values.
    pub derived_from: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Row {
    /// The week (`2031-W20`), day, or the project, tag or item id; `"none"`
    /// for unassigned work. It is a valid filter value for the same dimension.
    pub key: String,
    pub label: String,
    pub color: Option<String>,
    /// The days a week or day row covers, clipped to the range.
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub minutes: i64,
    pub sessions: i64,
    pub values: BTreeMap<String, Option<f64>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Totals {
    pub minutes: i64,
    pub sessions: i64,
    pub values: BTreeMap<String, Option<f64>>,
}

/// Reads the sessions in scope and builds the table. It never writes.
pub fn table(
    store: &Store,
    calendar: Calendar<'_>,
    range: Range,
    filter: Filter,
    group: Group,
) -> Result<Table, Error> {
    let scope = report_scope(store, &range, &filter)?;
    let ReportData {
        sessions,
        metrics,
        tags,
        windows,
    } = ReportData::load(
        store,
        calendar,
        &scope,
        group == Group::Tag,
        group == Group::Block,
    )?;
    let everything: Vec<_> = sessions.iter().map(|s| Sample::whole(&s.session)).collect();
    let buckets = match group {
        Group::Block => blocks::buckets(&sessions, &windows),
        Group::Week => dated(&everything, weeks(&range)),
        Group::Day => dated(&everything, days(&range)),
        Group::Project => keyed(&sessions, |s| vec![project_key(s)]),
        Group::Epic => keyed(&sessions, |s| vec![epic_key(s)]),
        Group::Tag => keyed(&sessions, |s| tag_keys(s, &tags)),
        Group::Item => keyed(&sessions, |s| vec![item_key(s)]),
    };
    let rows = buckets
        .into_iter()
        .map(|bucket| {
            let (minutes, count, values) = cells(&metrics, &bucket.sessions);
            Row {
                key: bucket.key,
                label: bucket.label,
                color: bucket.color,
                from: bucket.from,
                to: bucket.to,
                minutes,
                sessions: count,
                values,
            }
        })
        .collect();
    let (minutes, count, values) = cells(&metrics, &everything);
    Ok(Table {
        columns: columns(&metrics),
        rows,
        totals: Totals {
            minutes,
            sessions: count,
            values,
        },
        range,
        filter,
        group,
    })
}

/// Report inputs from one consistent snapshot. Only block grouping needs
/// prayer times, and only tag grouping needs the project's tags.
pub(super) struct ReportData {
    pub sessions: Vec<ReportSession>,
    pub metrics: Vec<Metric>,
    pub tags: BTreeMap<String, Vec<Tag>>,
    pub windows: blocks::Windows,
}

impl ReportData {
    pub(super) fn load(
        store: &Store,
        calendar: Calendar<'_>,
        scope: &salah_store::reports::SessionScope<'_>,
        by_tag: bool,
        by_block: bool,
    ) -> Result<Self, Error> {
        store.read_snapshot(|store| {
            let sessions = store.report_sessions(scope, None)?;
            let positions = match scope.project {
                Some(Choice::Is(id)) => store.project_metrics(id)?,
                _ => Vec::new(),
            };
            Ok(Self {
                metrics: metric_columns(store.metrics()?, &positions, &sessions),
                tags: if by_tag {
                    store.project_tags()?
                } else {
                    BTreeMap::new()
                },
                windows: if by_block {
                    calendar.windows(store, &sessions)?
                } else {
                    blocks::Windows::new()
                },
                sessions,
            })
        })
    }
}

/// Active metrics, and archived ones only when a session in scope recorded
/// them. A single-project filter orders them like the project's prompt.
pub(super) fn metric_columns(
    metrics: Vec<Metric>,
    positions: &[ProjectMetric],
    sessions: &[ReportSession],
) -> Vec<Metric> {
    let recorded: BTreeSet<&str> = sessions
        .iter()
        .flat_map(|s| s.session.values.iter().map(|v| v.metric_id.as_str()))
        .collect();
    let mut kept: Vec<Metric> = metrics
        .into_iter()
        .filter(|m| m.deleted_at.is_none() && (!m.archived || recorded.contains(m.id.as_str())))
        .collect();
    kept.sort_by_key(|m| {
        positions
            .iter()
            .position(|p| p.metric_id == m.id)
            .unwrap_or(usize::MAX)
    });
    kept
}

pub(super) fn columns(metrics: &[Metric]) -> Vec<Column> {
    let fixed = |id: &str, name: &str, unit: &str| Column {
        id: id.into(),
        name: name.into(),
        unit: unit.into(),
        kind: MetricKind::Count,
        rollup: Rollup::Sum,
        scale_max: None,
        archived: false,
        derived_from: None,
    };
    let mut columns = vec![
        fixed("minutes", "Minutes", "min"),
        fixed("sessions", "Sessions", "sessions"),
    ];
    for metric in metrics {
        columns.push(Column {
            id: metric.id.clone(),
            name: metric.name.clone(),
            unit: metric.unit.clone(),
            kind: metric.kind,
            rollup: metric.rollup,
            scale_max: metric.scale_max,
            archived: metric.archived,
            derived_from: None,
        });
        if metric.id == DISTRACTIONS {
            columns.push(Column {
                id: DISTRACTIONS_PER_HOUR.into(),
                name: format!("{} per hour", metric.name),
                unit: "per hour".into(),
                rollup: Rollup::Avg,
                derived_from: Some(DISTRACTIONS.into()),
                ..columns.last().cloned().expect("pushed above")
            });
        }
    }
    columns
}

/// A whole session or the portion of its recorded minutes assigned to a block.
#[derive(Clone, Copy)]
pub(super) struct Sample<'a> {
    pub session: &'a Session,
    pub minutes: Option<i64>,
}

impl<'a> Sample<'a> {
    pub(super) fn whole(session: &'a Session) -> Self {
        Self {
            session,
            minutes: session.minutes,
        }
    }

    fn metric(&self, metric: &Metric) -> Option<(f64, f64)> {
        let mut value = self.session.value(&metric.id)?;
        let weight = self.minutes.unwrap_or(1).max(1) as f64;
        if metric.rollup == Rollup::Sum && self.minutes != self.session.minutes {
            value *= weight / self.session.weight();
        }
        Some((value, weight))
    }
}

/// Minutes, session count and one value per metric column.
pub(super) fn cells(
    metrics: &[Metric],
    sessions: &[Sample<'_>],
) -> (i64, i64, BTreeMap<String, Option<f64>>) {
    let minutes = sessions.iter().filter_map(|s| s.minutes).sum();
    let mut values = BTreeMap::new();
    for metric in metrics {
        let value = aggregate(
            metric.rollup,
            sessions.iter().filter_map(|s| s.metric(metric)),
        );
        if metric.id == DISTRACTIONS {
            let rate = value
                .filter(|_| minutes > 0)
                .map(|sum| sum * 60.0 / minutes as f64);
            values.insert(DISTRACTIONS_PER_HOUR.to_owned(), rate);
        }
        values.insert(metric.id.clone(), value);
    }
    (minutes, sessions.len() as i64, values)
}

/// One table row before its numbers.
pub(super) struct Bucket<'a> {
    pub(super) key: String,
    pub(super) label: String,
    pub(super) color: Option<String>,
    pub(super) from: Option<NaiveDate>,
    pub(super) to: Option<NaiveDate>,
    pub(super) sessions: Vec<Sample<'a>>,
}

/// A row key of a project, tag or task table, with its label and colour.
pub(super) struct Key {
    pub(super) key: String,
    pub(super) label: String,
    pub(super) color: Option<String>,
}

impl Key {
    fn none(label: &str) -> Self {
        Self {
            key: NONE.into(),
            label: label.into(),
            color: None,
        }
    }
}

/// Every week or day of the range, including those without sessions.
pub(super) fn dated<'a>(
    sessions: &[Sample<'a>],
    spans: Vec<(String, String, NaiveDate, NaiveDate)>,
) -> Vec<Bucket<'a>> {
    let mut by_day = BTreeMap::<NaiveDate, Vec<Sample>>::new();
    for s in sessions {
        by_day.entry(s.session.block.day).or_default().push(*s);
    }
    spans
        .into_iter()
        .map(|(key, label, from, to)| Bucket {
            key,
            label,
            color: None,
            from: Some(from),
            to: Some(to),
            sessions: by_day
                .range(from..=to)
                .flat_map(|(_, s)| s.iter().copied())
                .collect(),
        })
        .collect()
}

/// Monday-first weeks clipped to the range, keyed by ISO week.
pub(super) fn weeks(range: &Range) -> Vec<(String, String, NaiveDate, NaiveDate)> {
    let mut weeks = Vec::new();
    let mut from = range.from;
    while from <= range.to {
        let monday = from - Days::new(u64::from(from.weekday().num_days_from_monday()));
        let to = (monday + Days::new(6)).min(range.to);
        let week = monday.iso_week();
        weeks.push((
            format!("{}-W{:02}", week.year(), week.week()),
            span_label(from, to),
            from,
            to,
        ));
        from = to + Days::new(1);
    }
    weeks
}

pub(super) fn days(range: &Range) -> Vec<(String, String, NaiveDate, NaiveDate)> {
    range
        .days()
        .into_iter()
        .map(|day| {
            (
                day.to_string(),
                day.format("%a %-d %b %Y").to_string(),
                day,
                day,
            )
        })
        .collect()
}

/// `12–18 May 2031`, `28 Apr – 4 May 2031`, `29 Dec 2031 – 4 Jan 2032`.
fn span_label(from: NaiveDate, to: NaiveDate) -> String {
    if from.year() != to.year() {
        format!("{} – {}", from.format("%-d %b %Y"), to.format("%-d %b %Y"))
    } else if from.month() != to.month() {
        format!("{} – {}", from.format("%-d %b"), to.format("%-d %b %Y"))
    } else if from != to {
        format!("{}–{}", from.format("%-d"), to.format("%-d %b %Y"))
    } else {
        from.format("%-d %b %Y").to_string()
    }
}

/// Rows of the keys that have sessions, most minutes first.
pub(super) fn keyed<'a>(
    sessions: &'a [ReportSession],
    keys_of: impl Fn(&'a ReportSession) -> Vec<Key>,
) -> Vec<Bucket<'a>> {
    let mut buckets = BTreeMap::<String, Bucket<'a>>::new();
    for s in sessions {
        for key in keys_of(s) {
            buckets
                .entry(key.key.clone())
                .or_insert_with(|| Bucket {
                    key: key.key,
                    label: key.label,
                    color: key.color,
                    from: None,
                    to: None,
                    sessions: Vec::new(),
                })
                .sessions
                .push(Sample::whole(&s.session));
        }
    }
    let mut rows: Vec<Bucket<'a>> = buckets.into_values().collect();
    rows.sort_by_cached_key(|b| {
        (
            std::cmp::Reverse(b.sessions.iter().filter_map(|s| s.minutes).sum::<i64>()),
            std::cmp::Reverse(b.sessions.len()),
            b.label.to_lowercase(),
            b.key.clone(),
        )
    });
    rows
}

pub(super) fn project_key(s: &ReportSession) -> Key {
    match &s.project {
        Some(project) => Key {
            key: project.id.clone(),
            label: project.name.clone(),
            color: Some(project.color.clone()),
        },
        None => Key::none("No project"),
    }
}

pub(super) fn epic_key(s: &ReportSession) -> Key {
    match (&s.session.epic_id, &s.epic_name) {
        (Some(id), Some(name)) => Key {
            key: id.clone(),
            label: name.clone(),
            color: s.project.as_ref().map(|p| p.color.clone()),
        },
        _ => Key::none("No epic"),
    }
}

/// Each live tag of the session's project, or the "No tag" row.
pub(super) fn tag_keys(s: &ReportSession, tags: &BTreeMap<String, Vec<Tag>>) -> Vec<Key> {
    let keys: Vec<Key> = s
        .session
        .project_id
        .as_ref()
        .and_then(|id| tags.get(id))
        .into_iter()
        .flatten()
        .map(|tag| Key {
            key: tag.id.clone(),
            label: tag.name.clone(),
            color: Some(tag.color.clone()),
        })
        .collect();
    if keys.is_empty() {
        vec![Key::none("No tag")]
    } else {
        keys
    }
}

/// The recorded task, coloured by the session's project; project-only work
/// is "No task".
pub(super) fn item_key(s: &ReportSession) -> Key {
    match &s.item {
        Some(item) => Key {
            key: item.id.clone(),
            label: item.title.clone(),
            color: s.project.as_ref().map(|p| p.color.clone()),
        },
        None => Key::none("No task"),
    }
}

#[cfg(test)]
pub(crate) mod tests;
