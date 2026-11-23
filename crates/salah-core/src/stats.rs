//! Statistics over a range of days, optionally narrowed to one project or to
//! the projects carrying one tag or to one task. Only finished sessions count.
//!
//! An `avg` rollup is weighted by session minutes; a tick weighs 1.

use crate::config::Config;
use crate::day::{self, Fulfilment, Snapshot, block_minutes, day_capacity, total_minutes};
use crate::{Store, projects, tags};
use chrono::NaiveDate;
use salah_domain::{
    day::BoundaryId,
    error::Error,
    tracking::{Metric, Rollup, Session, Tag},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// The longest range one request covers.
pub const MAX_DAYS: i64 = 730;

/// The pseudo-metric that leads `series`: logged minutes per day.
const MINUTES_SERIES: &str = "minutes";

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Filter {
    pub tag_id: Option<String>,
    pub project_id: Option<String>,
    pub item_id: Option<String>,
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
    /// Minutes of sessions outside the day's plan lines.
    pub unplanned_minutes: i64,
    /// Plan-line minutes over planned minutes, at most 1; `null` without a plan.
    pub fulfilment_ratio: Option<f64>,
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

/// Freezes the past days of the range, then summarises it.
pub fn stats(
    store: &mut Store,
    config: &Config,
    today: NaiveDate,
    range: Range,
    filter: Filter,
) -> Result<Stats, Error> {
    if range.to < range.from {
        return Err(Error::validation("to", "the range ends before it starts"));
    }
    if (range.to - range.from).num_days() >= MAX_DAYS {
        return Err(Error::validation(
            "from",
            format!("a range covers at most {MAX_DAYS} days"),
        ));
    }
    let days: Vec<NaiveDate> = range
        .from
        .iter_days()
        .take_while(|d| *d <= range.to)
        .collect();
    let scope = Scope::new(store, &filter)?;
    if range.from < today {
        store.write(|tx| {
            days.iter()
                .try_for_each(|d| day::freeze(tx, config, *d, today))
        })?;
    }
    let ledger = Ledger::load(store, config, today, days, scope)?;
    Ok(Stats {
        days: ledger.day_stats(),
        series: ledger.series(),
        projects: ledger.project_stats(),
        tags: ledger.tag_stats(),
        blocks: ledger.block_stats(),
        totals: ledger.totals(),
        range,
        filter,
    })
}

/// Everything in range that the filter keeps.
struct Ledger {
    days: Vec<(NaiveDate, Snapshot)>,
    sessions: Vec<Session>,
    scope: Scope,
    metrics: Vec<Metric>,
    projects: Vec<projects::ProjectView>,
    tags: Vec<Tag>,
}

impl Ledger {
    fn load(
        store: &Store,
        config: &Config,
        today: NaiveDate,
        days: Vec<NaiveDate>,
        scope: Scope,
    ) -> Result<Self, Error> {
        let (Some(first), Some(last)) = (days.first(), days.last()) else {
            return Err(Error::validation("from", "the range is empty"));
        };
        let sessions = store
            .sessions_between_for_item(*first, *last, scope.item_id.as_deref())?
            .into_iter()
            .filter(|s| s.is_live() && !s.is_running() && scope.covers(s.project_id.as_deref()))
            .collect();
        let days = days
            .into_iter()
            .map(|d| Ok((d, day::snapshot(store, config, d, today)?)))
            .collect::<Result<_, Error>>()?;
        Ok(Self {
            days,
            sessions,
            scope,
            metrics: store
                .metrics()?
                .into_iter()
                .filter(|m| m.deleted_at.is_none())
                .collect(),
            projects: projects::list(store, None)?,
            tags: tags::list(store)?,
        })
    }

    fn on(&self, day: NaiveDate) -> impl Iterator<Item = &Session> {
        self.sessions.iter().filter(move |s| s.block.day == day)
    }

    fn day_stats(&self) -> Vec<DayStat> {
        self.days
            .iter()
            .map(|(day, snapshot)| {
                let sessions: Vec<Session> = self.on(*day).cloned().collect();
                let day_plan = Fulfilment::of(&snapshot.plan, &sessions);
                // Project filters narrow the plan used for the ratio. Moving
                // a plan line to a different project must not turn a session
                // attached to that line into unplanned work.
                let unplanned_minutes = day_plan.unplanned_minutes;
                let fulfilment = if self.scope.projects.is_some() || self.scope.item_id.is_some() {
                    let lines = snapshot
                        .plan
                        .iter()
                        .filter(|line| {
                            self.scope.covers(line.project_id.as_deref())
                                && self
                                    .scope
                                    .item_id
                                    .as_ref()
                                    .is_none_or(|id| id == &line.item_id)
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    Fulfilment::of(&lines, &sessions)
                } else {
                    day_plan
                };
                DayStat {
                    day: *day,
                    minutes: total_minutes(&sessions),
                    capacity_minutes: day_capacity(&snapshot.blocks),
                    planned_minutes: fulfilment.planned_minutes,
                    unplanned_minutes,
                    fulfilment_ratio: fulfilment.ratio(),
                }
            })
            .collect()
    }

    /// Minutes first, then one series per metric.
    fn series(&self) -> Vec<Series> {
        let minutes = Series {
            metric_id: MINUTES_SERIES.into(),
            name: "Minutes".into(),
            unit: "min".into(),
            rollup: Rollup::Sum,
            points: self
                .days
                .iter()
                .map(|(day, _)| Point {
                    day: *day,
                    value: Some(total_minutes(self.on(*day)) as f64),
                })
                .collect(),
        };
        let metrics = self.metrics.iter().map(|metric| Series {
            metric_id: metric.id.clone(),
            name: metric.name.clone(),
            unit: metric.unit.clone(),
            rollup: metric.rollup,
            points: self
                .days
                .iter()
                .map(|(day, _)| Point {
                    day: *day,
                    value: rollup(metric.rollup, self.on(*day), &metric.id),
                })
                .collect(),
        });
        std::iter::once(minutes).chain(metrics).collect()
    }

    /// Projects with at least one session, each metric rolled up its own way.
    fn project_stats(&self) -> Vec<ProjectStat> {
        self.projects
            .iter()
            .filter_map(|view| {
                let project = &view.project;
                let mine: Vec<&Session> = self
                    .sessions
                    .iter()
                    .filter(|s| s.project_id.as_deref() == Some(project.id.as_str()))
                    .collect();
                if mine.is_empty() {
                    return None;
                }
                let recorded: BTreeSet<&str> = mine
                    .iter()
                    .flat_map(|s| s.values.iter().map(|v| v.metric_id.as_str()))
                    .collect();
                let metrics = self
                    .metrics
                    .iter()
                    .filter(|m| recorded.contains(m.id.as_str()))
                    .filter_map(|m| {
                        Some((m.id.clone(), rollup(m.rollup, mine.iter().copied(), &m.id)?))
                    })
                    .collect();
                Some(ProjectStat {
                    id: project.id.clone(),
                    name: project.name.clone(),
                    color: project.color.clone(),
                    minutes: total_minutes(mine.iter().copied()),
                    sessions: mine.len() as i64,
                    metrics,
                })
            })
            .collect()
    }

    fn tag_stats(&self) -> Vec<TagStat> {
        self.tags
            .iter()
            .map(|tag| {
                let tagged: BTreeSet<&str> = self
                    .projects
                    .iter()
                    .filter(|p| p.project.tag_ids.contains(&tag.id))
                    .map(|p| p.project.id.as_str())
                    .collect();
                TagStat {
                    id: tag.id.clone(),
                    name: tag.name.clone(),
                    color: tag.color.clone(),
                    minutes: total_minutes(
                        self.sessions.iter().filter(|s| {
                            s.project_id.as_deref().is_some_and(|p| tagged.contains(p))
                        }),
                    ),
                }
            })
            .collect()
    }

    fn block_stats(&self) -> Vec<BlockStat> {
        BoundaryId::AWAKE
            .into_iter()
            .map(|block| {
                let inside = || self.sessions.iter().filter(move |s| s.block.start == block);
                BlockStat {
                    block,
                    minutes: total_minutes(inside()),
                    capacity_minutes: self
                        .days
                        .iter()
                        .flat_map(|(_, snapshot)| &snapshot.blocks)
                        .filter(|b| b.block == block)
                        .map(block_minutes)
                        .sum(),
                    avg_focus: rollup(Rollup::Avg, inside(), "focus"),
                }
            })
            .collect()
    }

    fn totals(&self) -> Totals {
        Totals {
            minutes: total_minutes(&self.sessions),
            sessions: self.sessions.len() as i64,
            active_days: self
                .days
                .iter()
                .filter(|(day, _)| self.on(*day).next().is_some())
                .count() as i64,
            avg_focus: rollup(Rollup::Avg, self.sessions.iter(), "focus"),
            avg_quality: rollup(Rollup::Avg, self.sessions.iter(), "quality"),
        }
    }
}

/// A metric's values over `sessions`, summed or minute-weighted; `None`
/// when no session recorded it.
fn rollup<'a>(
    kind: Rollup,
    sessions: impl Iterator<Item = &'a Session>,
    metric_id: &str,
) -> Option<f64> {
    let mut samples = sessions.filter_map(|s| Some((s.value(metric_id)?, s.weight())));
    let (first, mut weight) = samples.next()?;
    Some(match kind {
        Rollup::Sum => samples.fold(first, |sum, (value, _)| sum + value),
        Rollup::Avg => samples.fold(first, |mean, (value, minutes)| {
            weight += minutes;
            mean + (value - mean) * (minutes / weight)
        }),
    })
}

/// The projects a filter keeps; `None` keeps everything.
struct Scope {
    projects: Option<BTreeSet<String>>,
    item_id: Option<String>,
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
        if let Some(id) = &filter.item_id
            && store.item(id)?.is_none()
        {
            return Err(crate::guard::not_found("item", id));
        }
        Ok(Self {
            projects,
            item_id: filter.item_id.clone(),
        })
    }

    fn covers(&self, project_id: Option<&str>) -> bool {
        match &self.projects {
            None => true,
            Some(chosen) => project_id.is_some_and(|id| chosen.contains(id)),
        }
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
                log(tx, &gym.id, None, 2.0, 22);
                Ok(())
            })
            .unwrap();
        let range = || Range {
            from: date(20),
            to: date(22),
        };
        let all = stats(&mut store, &config(), date(22), range(), Filter::default()).unwrap();
        assert_eq!(
            (
                all.totals.minutes,
                all.totals.sessions,
                all.totals.active_days
            ),
            (40, 4, 3)
        );
        // (5·30 + 2·1 + 1·10 + 2·1) / 42
        assert!((all.totals.avg_focus.unwrap() - 164.0 / 42.0).abs() < 1e-9);
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
            &mut store,
            &config(),
            date(22),
            range(),
            Filter {
                tag_id: Some(tag.id),
                project_id: None,
                item_id: None,
            },
        )
        .unwrap();
        assert_eq!((tagged.totals.minutes, tagged.projects.len()), (30, 1));
        assert!(
            stats(
                &mut store,
                &config(),
                date(22),
                Range {
                    from: date(21),
                    to: date(20)
                },
                Filter::default()
            )
            .is_err()
        );
    }

    #[test]
    fn running_sessions_are_not_counted_and_the_ratio_is_capped_or_null() {
        let mut store = Store::open_in_memory().unwrap();
        let quran = project(&mut store, "Quran");
        let read = store
            .write(|tx| {
                crate::items::add(
                    tx,
                    &config(),
                    at(20, 6, 0),
                    ids(),
                    crate::items::NewItem {
                        title: "Read".into(),
                        project_id: Some(quran.id.clone()),
                        planned_minutes: Some(10),
                        ..Default::default()
                    },
                )
            })
            .unwrap();
        store
            .write(|tx| {
                sessions::log(
                    tx,
                    &config(),
                    at(20, 13, 0),
                    ids(),
                    NewLog {
                        target: Target {
                            item_id: Some(read.id.clone()),
                            project_id: None,
                        },
                        minutes: Some(25),
                        values: Values::from([("focus".into(), 4.0), ("quality".into(), 3.0)]),
                        ..NewLog::default()
                    },
                )?;
                sessions::start(
                    tx,
                    &config(),
                    at(20, 14, 0),
                    ids(),
                    Target {
                        item_id: None,
                        project_id: Some(quran.id.clone()),
                    },
                )
            })
            .unwrap();
        let range = Range {
            from: date(20),
            to: date(21),
        };
        let stats = stats(&mut store, &config(), date(21), range, Filter::default()).unwrap();
        assert_eq!(stats.totals.sessions, 1, "the running timer is not counted");
        assert_eq!(
            stats.days[0].fulfilment_ratio,
            Some(1.0),
            "25 of 10 minutes caps at 1"
        );
        assert_eq!(
            stats.days[1].fulfilment_ratio, None,
            "nothing was planned on day 21"
        );
        assert!(
            stats.days[0].capacity_minutes < 1440,
            "the night is not capacity"
        );
    }
}
