//! Statistics over a range of days, optionally narrowed to one project or to
//! the projects carrying one tag.
//!
//! An `avg` rollup is weighted by session minutes; a tick weighs 1.

use crate::config::Config;
use crate::day::{self, total_minutes};
use crate::{Store, projects, tags};
use chrono::NaiveDate;
use salah_domain::{
    day::BoundaryId,
    error::Error,
    tracking::{DayBlock, Rollup, Session},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// The longest range one request covers.
pub const MAX_DAYS: i64 = 366;

/// The pseudo-metric that leads `series`: logged minutes per day.
const MINUTES_SERIES: &str = "minutes";

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Filter {
    pub tag_id: Option<String>,
    pub project_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Range {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Stats {
    pub range: Range,
    pub filter: Filter,
    pub days: Vec<DayStat>,
    pub series: Vec<Series>,
    pub projects: Vec<ProjectStat>,
    pub tags: Vec<TagStat>,
    pub blocks: Vec<BlockStat>,
    pub totals: Totals,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DayStat {
    pub day: NaiveDate,
    pub minutes: i64,
    pub capacity_minutes: i64,
    pub planned_minutes: i64,
    /// Logged over planned minutes; 0 when nothing was planned.
    pub fulfilment_ratio: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Series {
    pub metric_id: String,
    pub name: String,
    pub unit: String,
    pub rollup: Rollup,
    pub points: Vec<Point>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Point {
    pub day: NaiveDate,
    pub value: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectStat {
    pub id: String,
    pub name: String,
    pub color: String,
    pub minutes: i64,
    pub sessions: i64,
    pub metrics: BTreeMap<String, f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TagStat {
    pub id: String,
    pub name: String,
    pub color: String,
    pub minutes: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BlockStat {
    pub block: BoundaryId,
    pub minutes: i64,
    pub capacity_minutes: i64,
    pub avg_focus: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Totals {
    pub minutes: i64,
    pub sessions: i64,
    pub active_days: i64,
    pub avg_focus: Option<f64>,
    pub avg_quality: Option<f64>,
}

pub fn stats(store: &Store, config: &Config, range: Range, filter: Filter) -> Result<Stats, Error> {
    if range.to < range.from {
        return Err(Error::validation("to", "the range ends before it starts"));
    }
    if (range.to - range.from).num_days() >= MAX_DAYS {
        return Err(Error::validation(
            "from",
            format!("a range covers at most {MAX_DAYS} days"),
        ));
    }
    let scope = Scope::new(store, &filter)?;
    let sessions: Vec<Session> = store
        .sessions_between(range.from, range.to)?
        .into_iter()
        .filter(|s| s.is_live() && scope.covers(s.project_id.as_deref()))
        .collect();
    let days: Vec<NaiveDate> = range
        .from
        .iter_days()
        .take_while(|d| *d <= range.to)
        .collect();
    let blocks: Vec<DayBlock> = days
        .iter()
        .map(|d| day::blocks(store, config, *d))
        .collect::<Result<Vec<_>, _>>()?
        .concat();
    let on = |day: NaiveDate| sessions.iter().filter(move |s| s.block.day == day);

    let mut day_stats = Vec::with_capacity(days.len());
    for day in &days {
        let minutes = total_minutes(on(*day));
        let planned_minutes = scope.planned_minutes(store, *day)?;
        day_stats.push(DayStat {
            day: *day,
            minutes,
            capacity_minutes: capacity(blocks.iter().filter(|b| b.day == *day)),
            planned_minutes,
            fulfilment_ratio: if planned_minutes > 0 {
                minutes as f64 / planned_minutes as f64
            } else {
                0.0
            },
        });
    }

    let mut series = vec![Series {
        metric_id: MINUTES_SERIES.into(),
        name: "Minutes".into(),
        unit: "min".into(),
        rollup: Rollup::Sum,
        points: day_stats
            .iter()
            .map(|d| Point {
                day: d.day,
                value: Some(d.minutes as f64),
            })
            .collect(),
    }];
    for metric in store
        .metrics()?
        .into_iter()
        .filter(|m| m.deleted_at.is_none())
    {
        let points = days
            .iter()
            .map(|day| Point {
                day: *day,
                value: rollup(metric.rollup, on(*day), &metric.id),
            })
            .collect();
        series.push(Series {
            metric_id: metric.id,
            name: metric.name,
            unit: metric.unit,
            rollup: metric.rollup,
            points,
        });
    }

    let rollups: BTreeMap<String, Rollup> = store
        .metrics()?
        .into_iter()
        .map(|m| (m.id, m.rollup))
        .collect();
    let mut project_stats = Vec::new();
    for project in projects::list(store, None)? {
        let mine: Vec<&Session> = sessions
            .iter()
            .filter(|s| s.project_id.as_deref() == Some(project.project.id.as_str()))
            .collect();
        if mine.is_empty() {
            continue;
        }
        let metric_ids: BTreeSet<&str> = mine
            .iter()
            .flat_map(|s| s.values.iter().map(|v| v.metric_id.as_str()))
            .collect();
        let metrics = metric_ids
            .into_iter()
            .filter_map(|id| {
                let kind = rollups.get(id).copied().unwrap_or(Rollup::Avg);
                Some((id.to_owned(), rollup(kind, mine.iter().copied(), id)?))
            })
            .collect();
        project_stats.push(ProjectStat {
            minutes: total_minutes(mine.iter().copied()),
            sessions: mine.len() as i64,
            metrics,
            id: project.project.id,
            name: project.project.name,
            color: project.project.color,
        });
    }

    let all_projects = projects::list(store, None)?;
    let tag_stats = tags::list(store)?
        .into_iter()
        .map(|tag| {
            let tagged: BTreeSet<&str> = all_projects
                .iter()
                .filter(|p| p.project.tag_ids.contains(&tag.id))
                .map(|p| p.project.id.as_str())
                .collect();
            TagStat {
                minutes: total_minutes(
                    sessions
                        .iter()
                        .filter(|s| s.project_id.as_deref().is_some_and(|p| tagged.contains(p))),
                ),
                id: tag.id,
                name: tag.name,
                color: tag.color,
            }
        })
        .collect();

    let block_stats = BoundaryId::AWAKE
        .into_iter()
        .map(|block| {
            let inside = || sessions.iter().filter(move |s| s.block.start == block);
            BlockStat {
                block,
                minutes: total_minutes(inside()),
                capacity_minutes: capacity(blocks.iter().filter(|b| b.block == block)),
                avg_focus: rollup(Rollup::Avg, inside(), "focus"),
            }
        })
        .collect();

    Ok(Stats {
        totals: Totals {
            minutes: total_minutes(sessions.iter()),
            sessions: sessions.len() as i64,
            active_days: day_stats.iter().filter(|d| d.minutes > 0).count() as i64,
            avg_focus: rollup(Rollup::Avg, sessions.iter(), "focus"),
            avg_quality: rollup(Rollup::Avg, sessions.iter(), "quality"),
        },
        range,
        filter,
        days: day_stats,
        series,
        projects: project_stats,
        tags: tag_stats,
        blocks: block_stats,
    })
}

/// A metric's values over `sessions`, summed or minute-weighted; `None`
/// when no session recorded it.
fn rollup<'a>(
    kind: Rollup,
    sessions: impl Iterator<Item = &'a Session>,
    metric_id: &str,
) -> Option<f64> {
    let samples: Vec<(f64, f64)> = sessions
        .filter_map(|s| Some((s.value(metric_id)?, s.weight())))
        .collect();
    if samples.is_empty() {
        return None;
    }
    Some(match kind {
        Rollup::Sum => samples.iter().map(|(value, _)| value).sum(),
        Rollup::Avg => {
            let weight: f64 = samples.iter().map(|(_, weight)| weight).sum();
            samples
                .iter()
                .map(|(value, weight)| value * weight)
                .sum::<f64>()
                / weight
        }
    })
}

fn capacity<'a>(blocks: impl Iterator<Item = &'a DayBlock>) -> i64 {
    blocks
        .map(|b| (b.ends_at - b.starts_at).num_minutes().max(0))
        .sum()
}

/// The projects a filter keeps; `None` keeps everything.
struct Scope {
    projects: Option<BTreeSet<String>>,
}

impl Scope {
    fn new(store: &Store, filter: &Filter) -> Result<Self, Error> {
        let mut projects: Option<BTreeSet<String>> = None;
        if let Some(id) = &filter.project_id {
            projects = Some(BTreeSet::from([projects::find(store, id)?.id]));
        }
        if let Some(id) = &filter.tag_id {
            let tag = tags::find(store, id)?;
            let tagged: BTreeSet<String> = store
                .projects()?
                .into_iter()
                .filter(|p| p.tag_ids.contains(&tag.id))
                .map(|p| p.id)
                .collect();
            projects = Some(match projects {
                Some(chosen) => chosen.intersection(&tagged).cloned().collect(),
                None => tagged,
            });
        }
        Ok(Self { projects })
    }

    fn covers(&self, project_id: Option<&str>) -> bool {
        match &self.projects {
            None => true,
            Some(chosen) => project_id.is_some_and(|id| chosen.contains(id)),
        }
    }

    fn planned_minutes(&self, store: &Store, day: NaiveDate) -> Result<i64, Error> {
        let mut planned = 0;
        for line in day::plan(store, day)? {
            if self.projects.is_some() {
                let project = store.item(&line.item_id)?.and_then(|i| i.project_id);
                if !self.covers(project.as_deref()) {
                    continue;
                }
            }
            planned += line.planned_minutes.unwrap_or(0);
        }
        Ok(planned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WriteTx;
    use crate::sessions::{self, NewLog, Target, Values};
    use crate::test_support::{at, config, date, ids, project};

    fn log(tx: &WriteTx<'_>, project_id: &str, minutes: Option<i64>, focus: f64, day: u32) {
        sessions::log(
            tx,
            &config(),
            at(day, 13, 0),
            ids(),
            NewLog {
                target: Target {
                    item_id: None,
                    project_id: Some(project_id.into()),
                },
                minutes,
                values: Values::from([("focus".into(), focus), ("quality".into(), 3.0)]),
                ..NewLog::default()
            },
        )
        .unwrap();
    }

    #[test]
    fn rollups_are_minute_weighted_and_filters_narrow_the_sessions() {
        let mut store = Store::open_in_memory().unwrap();
        let quran = project(&mut store, "Quran");
        let gym = project(&mut store, "Gym");
        let tag = store
            .write(|tx| {
                let tag = tags::add(tx, at(20, 9, 0), ids(), "Deen", "#00aa00")?;
                projects::edit(
                    tx,
                    at(20, 9, 0),
                    &quran.id,
                    projects::ProjectChanges {
                        tag_ids: Some(vec![tag.id.clone()]),
                        ..Default::default()
                    },
                    None,
                )?;
                Ok(tag)
            })
            .unwrap();
        store
            .write(|tx| {
                log(tx, &quran.id, Some(30), 5.0, 20);
                log(tx, &quran.id, None, 2.0, 20);
                log(tx, &gym.id, Some(10), 1.0, 21);
                Ok(())
            })
            .unwrap();
        let range = || Range {
            from: date(20),
            to: date(21),
        };
        let all = stats(&store, &config(), range(), Filter::default()).unwrap();
        assert_eq!(
            (
                all.totals.minutes,
                all.totals.sessions,
                all.totals.active_days
            ),
            (40, 3, 2)
        );
        // (5·30 + 2·1 + 1·10) / 41
        assert!((all.totals.avg_focus.unwrap() - 162.0 / 41.0).abs() < 1e-9);
        assert_eq!(all.series[0].metric_id, "minutes");
        assert_eq!(all.series[1].metric_id, "focus");
        assert!(all.days[0].capacity_minutes > 0);
        assert_eq!(all.projects.len(), 2);
        assert_eq!(all.tags[0].minutes, 30);
        let dhuhr = all
            .blocks
            .iter()
            .find(|b| b.block == BoundaryId::Dhuhr)
            .unwrap();
        assert_eq!(dhuhr.minutes, 40);

        let tagged = stats(
            &store,
            &config(),
            range(),
            Filter {
                tag_id: Some(tag.id),
                project_id: None,
            },
        )
        .unwrap();
        assert_eq!((tagged.totals.minutes, tagged.projects.len()), (30, 1));
        assert!(
            stats(
                &store,
                &config(),
                Range {
                    from: date(21),
                    to: date(20)
                },
                Filter::default()
            )
            .is_err()
        );
    }
}
