//! `stats.series`: the Trends line chart. One series per (split key x
//! measure), one point per day or Monday-first week of the range. Aggregation
//! is `stats.table`'s own (`table::cells`): the same weighting, rollups,
//! distractions per hour and `key: "none"` for unassigned work.
//!
//! A period without sessions has `value: null`, except minutes and sessions
//! which are 0, so a line shows a gap rather than a fake zero. With a split,
//! at most [`MAX_KEYS`] keys are kept (most minutes first); the rest merge
//! into one `other` series, counting each session once.

use super::table::{
    Bucket, Column, cells, columns, dated, days, epic_key, item_key, keyed, metric_columns,
    project_key, tag_keys, weeks,
};
use super::{Filter, Range, report_scope};
use crate::{Store, guard::not_found};
use chrono::NaiveDate;
use salah_domain::{error::Error, tracking::Session};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The most split keys one request returns, before the combined "Other".
pub const MAX_KEYS: usize = 8;
/// The key of the combined series.
pub const OTHER: &str = "other";
/// The key of the one series per measure when there is no split.
pub const ALL: &str = "all";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Interval {
    #[default]
    Day,
    Week,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Split {
    Project,
    Epic,
    Tag,
    Item,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Trend {
    pub range: Range,
    pub filter: Filter,
    pub interval: Interval,
    pub split: Option<Split>,
    /// The first day of each period, clipped to the range for the first week.
    pub periods: Vec<NaiveDate>,
    pub series: Vec<Series>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Series {
    /// The split key (`"none"` for unassigned work, `"other"` for the
    /// combined rest, `"all"` without a split).
    pub key: String,
    /// The measure's name without a split; the key's label with one measure;
    /// `key label · measure name` with several.
    pub label: String,
    pub color: Option<String>,
    pub measure: String,
    pub unit: String,
    pub scale_max: Option<i64>,
    /// One per period, in `periods` order.
    pub points: Vec<Point>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Point {
    pub value: Option<f64>,
    pub sessions: i64,
}

/// Reads the sessions in scope and builds the series. It never writes.
pub fn series(
    store: &Store,
    range: Range,
    filter: Filter,
    interval: Interval,
    split: Option<Split>,
    measures: &[String],
) -> Result<Trend, Error> {
    if measures.is_empty() {
        return Err(Error::validation("measures", "name at least one measure"));
    }
    let scope = report_scope(store, &range, &filter)?;
    let (sessions, metrics, tags) = store.read_snapshot(|store| {
        let tags = match split {
            Some(Split::Tag) => store.project_tags()?,
            _ => BTreeMap::new(),
        };
        Ok((store.report_sessions(&scope, None)?, store.metrics()?, tags))
    })?;
    let metrics = metric_columns(metrics, &[], &sessions);
    let known = columns(&metrics);
    let mut chosen: Vec<&Column> = Vec::new();
    for id in measures {
        let column = known
            .iter()
            .find(|c| c.id == *id)
            .ok_or_else(|| not_found("metric", id))?;
        if !chosen.iter().any(|c| c.id == column.id) {
            chosen.push(column);
        }
    }
    let spans = match interval {
        Interval::Day => days(&range),
        Interval::Week => weeks(&range),
    };
    let everything: Vec<&Session> = sessions.iter().map(|s| &s.session).collect();
    let groups: Vec<Bucket<'_>> = match split {
        None => vec![Bucket {
            key: ALL.into(),
            label: String::new(),
            color: None,
            from: None,
            to: None,
            sessions: everything,
        }],
        Some(split) => top(match split {
            Split::Project => keyed(&sessions, |s| vec![project_key(s)]),
            Split::Epic => keyed(&sessions, |s| vec![epic_key(s)]),
            Split::Tag => keyed(&sessions, |s| tag_keys(s, &tags)),
            Split::Item => keyed(&sessions, |s| vec![item_key(s)]),
        }),
    };
    let mut result = Vec::new();
    for group in &groups {
        let periods: Vec<_> = dated(&group.sessions, spans.clone())
            .into_iter()
            .map(|p| cells(&metrics, &p.sessions))
            .collect();
        for column in &chosen {
            let label = match (split, chosen.len()) {
                (None, _) => column.name.clone(),
                (Some(_), 1) => group.label.clone(),
                (Some(_), _) => format!("{} · {}", group.label, column.name),
            };
            result.push(Series {
                key: group.key.clone(),
                label,
                color: group.color.clone(),
                measure: column.id.clone(),
                unit: column.unit.clone(),
                scale_max: column.scale_max,
                points: periods
                    .iter()
                    .map(|(minutes, count, values)| Point {
                        value: match column.id.as_str() {
                            "minutes" => Some(*minutes as f64),
                            "sessions" => Some(*count as f64),
                            id => values.get(id).copied().flatten(),
                        },
                        sessions: *count,
                    })
                    .collect(),
            });
        }
    }
    Ok(Trend {
        periods: spans.iter().map(|(_, _, from, _)| *from).collect(),
        range,
        filter,
        interval,
        split,
        series: result,
    })
}

/// The first [`MAX_KEYS`] keys (already most minutes first) and one combined
/// `other` group of the rest, each session once.
fn top(mut groups: Vec<Bucket<'_>>) -> Vec<Bucket<'_>> {
    if groups.len() > MAX_KEYS {
        let mut seen = BTreeSet::new();
        let sessions = groups
            .split_off(MAX_KEYS)
            .into_iter()
            .flat_map(|g| g.sessions)
            .filter(|s| seen.insert(s.id.clone()))
            .collect();
        groups.push(Bucket {
            key: OTHER.into(),
            label: "Other".into(),
            color: None,
            from: None,
            to: None,
            sessions,
        });
    }
    groups
}

#[cfg(test)]
mod tests;
