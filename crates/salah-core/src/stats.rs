//! Statistics over a range of days, optionally narrowed to one project or to
//! the projects carrying one tag or to one task. Only finished sessions count.
//!
//! An `avg` rollup is weighted by session minutes; a tick weighs 1.

use crate::config::Config;
use crate::day::{self, Fulfilment, Snapshot, block_capacity, day_capacity, total_minutes};
use crate::{Store, projects, tags};
use chrono::NaiveDate;
use measures::Measures;
use salah_domain::{
    day::BoundaryId,
    error::Error,
    tracking::{Metric, Rollup, Session, Tag},
};
use salah_store::reports::{Choice, ReportSession, SessionScope};
use serde::Serialize;
use std::collections::BTreeMap;

mod blocks;
pub use blocks::Calendar;
pub mod measures;
pub mod series;
pub mod table;

/// The longest range one request covers.
pub const MAX_DAYS: i64 = 730;

/// The pseudo-metric that leads `series`: logged minutes per day.
const MINUTES_SERIES: &str = "minutes";

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Filter {
    pub tag_id: Option<String>,
    pub project_id: Option<String>,
    pub epic_id: Option<String>,
    pub item_id: Option<String>,
    /// Narrows the aggregated sessions by minutes and metric values.
    #[serde(flatten)]
    pub measures: Measures,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Range {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

impl Range {
    /// Rejects a reversed range or one longer than [`MAX_DAYS`].
    pub fn check(&self) -> Result<(), Error> {
        if self.to < self.from {
            return Err(Error::validation("to", "the range ends before it starts"));
        }
        if (self.to - self.from).num_days() >= MAX_DAYS {
            return Err(Error::validation(
                "from",
                format!("a range covers at most {MAX_DAYS} days"),
            ));
        }
        Ok(())
    }

    pub(crate) fn days(&self) -> Vec<NaiveDate> {
        let to = self.to;
        self.from.iter_days().take_while(|d| *d <= to).collect()
    }
}

/// The filter value that selects unassigned work in reports.
pub const NONE: &str = "none";

/// The store scope of a report's range and filter. Each named id must exist
/// (a removed item still matches its historical sessions); [`NONE`] selects
/// work without a project, tag or item.
pub(crate) fn report_scope<'a>(
    store: &Store,
    range: &Range,
    filter: &'a Filter,
) -> Result<SessionScope<'a>, Error> {
    range.check()?;
    let choice = |id: &'a Option<String>, exists: &dyn Fn(&str) -> Result<(), Error>| {
        id.as_deref()
            .map(|id| match id {
                NONE => Ok(Choice::Unassigned),
                id => exists(id).map(|()| Choice::Is(id)),
            })
            .transpose()
    };
    Ok(SessionScope {
        from: range.from,
        to: range.to,
        project: choice(&filter.project_id, &|id| {
            projects::find(store, id).map(drop)
        })?,
        epic: choice(&filter.epic_id, &|id| {
            store
                .epic(id)?
                .map(drop)
                .ok_or_else(|| crate::guard::not_found("epic", id))
        })?,
        tag: choice(&filter.tag_id, &|id| tags::find(store, id).map(drop))?,
        item: choice(&filter.item_id, &|id| match store.item(id)? {
            Some(_) => Ok(()),
            None => Err(crate::guard::not_found("item", id)),
        })?,
        measures: filter.measures.resolve(store)?,
    })
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Stats {
    pub range: Range,
    pub filter: Filter,
    pub days: Vec<DayStat>,
    pub series: Vec<Series>,
    pub projects: Vec<ProjectStat>,
    pub epics: Vec<EpicStat>,
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
    pub color: Option<String>,
    pub minutes: i64,
    pub sessions: i64,
    pub metrics: BTreeMap<String, f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EpicStat {
    pub id: String,
    pub name: String,
    pub project_id: Option<String>,
    pub minutes: i64,
    pub sessions: i64,
    pub metrics: BTreeMap<String, f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TagStat {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
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
    range.check()?;
    let days = range.days();
    let (scope, sessions, tags) = store.read_snapshot(|store| {
        let scope = report_scope(store, &range, &filter)?;
        let sessions = store.report_sessions(&scope, None)?;
        let scope = Scope::new(store, &scope)?;
        let tags = tag_breakdown(tags::list(store)?, &scope.tags, &sessions);
        Ok((scope, sessions, tags))
    })?;
    if range.from < today {
        store.write(|tx| {
            days.iter()
                .try_for_each(|d| day::freeze(tx, config, *d, today))
        })?;
    }
    let ledger = Ledger::load(store, config, today, days, scope, sessions)?;
    Ok(Stats {
        days: ledger.day_stats(),
        series: ledger.series(),
        projects: ledger.project_stats(),
        tags,
        blocks: ledger.block_stats(),
        totals: ledger.totals(),
        epics: ledger.epic_stats,
        range,
        filter,
    })
}

fn tag_breakdown(
    tags: Vec<Tag>,
    project_tags: &BTreeMap<String, Vec<String>>,
    sessions: &[ReportSession],
) -> Vec<TagStat> {
    let mut minutes = BTreeMap::<&str, i64>::new();
    for row in sessions {
        let session = &row.session;
        let worked = session.minutes.unwrap_or(0);
        match session
            .project_id
            .as_ref()
            .and_then(|id| project_tags.get(id))
        {
            Some(ids) => {
                for id in ids {
                    *minutes.entry(id).or_default() += worked;
                }
            }
            None => *minutes.entry(NONE).or_default() += worked,
        }
    }
    let mut rows: Vec<_> = tags
        .into_iter()
        .map(|tag| TagStat {
            minutes: minutes.get(tag.id.as_str()).copied().unwrap_or(0),
            id: tag.id,
            name: tag.name,
            color: Some(tag.color),
        })
        .collect();
    if let Some(&minutes) = minutes.get(NONE) {
        rows.push(TagStat {
            id: NONE.into(),
            name: "No tag".into(),
            color: None,
            minutes,
        });
    }
    rows
}

/// Everything in range that the filter keeps.
struct Ledger {
    days: Vec<(NaiveDate, Snapshot)>,
    sessions: Vec<Session>,
    epic_stats: Vec<EpicStat>,
    scope: Scope,
    metrics: Vec<Metric>,
    projects: Vec<salah_domain::tracking::Project>,
}

impl Ledger {
    fn load(
        store: &Store,
        config: &Config,
        today: NaiveDate,
        days: Vec<NaiveDate>,
        scope: Scope,
        sessions: Vec<ReportSession>,
    ) -> Result<Self, Error> {
        if days.is_empty() {
            return Err(Error::validation("from", "the range is empty"));
        }
        let metrics: Vec<_> = store
            .metrics()?
            .into_iter()
            .filter(|m| m.deleted_at.is_none())
            .collect();
        let projects = store.projects()?;
        let epic_stats = epic_breakdown(&sessions, &metrics, &projects);
        let sessions = sessions.into_iter().map(|s| s.session).collect();
        let days = days
            .into_iter()
            .map(|d| Ok((d, day::snapshot(store, config, d, today)?)))
            .collect::<Result<_, Error>>()?;
        Ok(Self {
            days,
            sessions,
            scope,
            metrics,
            epic_stats,
            projects,
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
                let fulfilment = if self.scope.narrows_plan() {
                    let lines = snapshot
                        .plan
                        .iter()
                        .filter(|line| {
                            self.scope.keeps(
                                line.project_id.as_deref(),
                                line.epic_id.as_deref(),
                                &line.item_id,
                            )
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
        let mut groups = BTreeMap::<_, Vec<_>>::new();
        for session in &self.sessions {
            groups
                .entry(session.project_id.as_deref())
                .or_default()
                .push(session);
        }
        // Keep the existing project order. The final bucket is work without a project.
        self.projects
            .iter()
            .map(Some)
            .chain(std::iter::once(None))
            .filter_map(|project| {
                let id = project.map(|p| p.id.as_str());
                let mine = groups.remove(&id)?;
                Some(ProjectStat {
                    id: id.unwrap_or(NONE).to_owned(),
                    name: project.map_or("No project", |p| &p.name).to_owned(),
                    color: project.map(|p| p.color.clone()),
                    minutes: total_minutes(mine.iter().copied()),
                    sessions: mine.len() as i64,
                    metrics: recorded_metrics(&self.metrics, &mine),
                })
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
                        .map(block_capacity)
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

fn recorded_metrics(metrics: &[Metric], sessions: &[&Session]) -> BTreeMap<String, f64> {
    metrics
        .iter()
        .filter_map(|m| {
            Some((
                m.id.clone(),
                rollup(m.rollup, sessions.iter().copied(), &m.id)?,
            ))
        })
        .collect()
}

/// Each project's epics (most minutes first) and then its "No epic" bucket, in
/// project order, with work without a project last.
fn epic_breakdown(
    sessions: &[ReportSession],
    metrics: &[Metric],
    projects: &[salah_domain::tracking::Project],
) -> Vec<EpicStat> {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for row in sessions {
        groups
            .entry((
                row.session.project_id.as_deref(),
                row.session.epic_id.as_deref(),
                row.epic_name.as_deref(),
            ))
            .or_default()
            .push(&row.session);
    }
    let mut stats: Vec<EpicStat> = groups
        .into_iter()
        .map(|((project, epic, name), sessions)| EpicStat {
            id: epic.unwrap_or(NONE).to_owned(),
            name: name.unwrap_or("No epic").to_owned(),
            project_id: project.map(str::to_owned),
            minutes: total_minutes(sessions.iter().copied()),
            sessions: sessions.len() as i64,
            metrics: recorded_metrics(metrics, &sessions),
        })
        .collect();
    let rank = |project: Option<&str>| {
        project.map_or(projects.len(), |id| {
            projects
                .iter()
                .position(|p| p.id == id)
                .unwrap_or(projects.len())
        })
    };
    stats.sort_by(|a, b| {
        (rank(a.project_id.as_deref()), a.project_id.is_none())
            .cmp(&(rank(b.project_id.as_deref()), b.project_id.is_none()))
            .then((a.id == NONE).cmp(&(b.id == NONE)))
            .then(b.minutes.cmp(&a.minutes))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.id.cmp(&b.id))
    });
    stats
}

/// A metric's values over `sessions`, summed or minute-weighted; `None`
/// when no session recorded it.
pub(crate) fn rollup<'a>(
    kind: Rollup,
    sessions: impl Iterator<Item = &'a Session>,
    metric_id: &str,
) -> Option<f64> {
    aggregate(
        kind,
        sessions.filter_map(|s| Some((s.value(metric_id)?, s.weight()))),
    )
}

fn aggregate(kind: Rollup, mut samples: impl Iterator<Item = (f64, f64)>) -> Option<f64> {
    let (first, mut weight) = samples.next()?;
    Some(match kind {
        Rollup::Sum => samples.fold(first, |sum, (value, _)| sum + value),
        Rollup::Avg => samples.fold(first, |mean, (value, minutes)| {
            weight += minutes;
            mean + (value - mean) * (minutes / weight)
        }),
    })
}

/// One filter dimension of the plan lines a report keeps.
enum Pick {
    Unassigned,
    Is(String),
}

impl Pick {
    fn of(choice: Option<Choice<'_>>) -> Option<Self> {
        choice.map(|choice| match choice {
            Choice::Unassigned => Self::Unassigned,
            Choice::Is(id) => Self::Is(id.to_owned()),
        })
    }
}

/// Which plan lines a report's project, tag and task filters keep, so the
/// fulfilment ratio compares like with like. Duration and metric bounds do
/// not apply: they narrow sessions, never the plan.
struct Scope {
    project: Option<Pick>,
    epic: Option<Pick>,
    tag: Option<Pick>,
    item: Option<Pick>,
    /// Each project's live tag ids.
    tags: BTreeMap<String, Vec<String>>,
}

impl Scope {
    fn new(store: &Store, scope: &SessionScope<'_>) -> Result<Self, Error> {
        Ok(Self {
            project: Pick::of(scope.project),
            epic: Pick::of(scope.epic),
            tag: Pick::of(scope.tag),
            item: Pick::of(scope.item),
            tags: store
                .project_tags()?
                .into_iter()
                .map(|(project, tags)| (project, tags.into_iter().map(|t| t.id).collect()))
                .collect(),
        })
    }

    fn narrows_plan(&self) -> bool {
        self.project.is_some() || self.epic.is_some() || self.tag.is_some() || self.item.is_some()
    }

    /// Plan lines always name a task, so `none` for tasks keeps no line.
    fn keeps(&self, project_id: Option<&str>, epic_id: Option<&str>, item_id: &str) -> bool {
        let tags = project_id.and_then(|id| self.tags.get(id));
        self.project.as_ref().is_none_or(|pick| match pick {
            Pick::Unassigned => project_id.is_none(),
            Pick::Is(id) => project_id == Some(id.as_str()),
        }) && self.epic.as_ref().is_none_or(|pick| match pick {
            Pick::Unassigned => epic_id.is_none(),
            Pick::Is(id) => epic_id == Some(id.as_str()),
        }) && self.tag.as_ref().is_none_or(|pick| match pick {
            Pick::Unassigned => tags.is_none_or(Vec::is_empty),
            Pick::Is(id) => tags.is_some_and(|tags| tags.contains(id)),
        }) && self.item.as_ref().is_none_or(|pick| match pick {
            Pick::Unassigned => false,
            Pick::Is(id) => id == item_id,
        })
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
                    epic_id: None,
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
                ..Filter::default()
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
                            epic_id: None,
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
                        epic_id: None,
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
