//! A day's plan: its prayer blocks, the items planned in them, the sessions
//! recorded, and how much of the plan was fulfilled.
//!
//! The block times and plan lines of a day are snapshotted the first time the
//! day is read or written. Today's and future plans follow item edits; a past
//! day keeps its snapshot.

use crate::config::Config;
use crate::marks::live_marks;
use crate::sessions::SessionView;
use crate::status::format_minutes;
use crate::{Store, WriteTx, items};
use chrono::{DateTime, NaiveDate, Utc};
use salah_domain::{
    day::{BlockKey, BoundaryId},
    error::Error,
    item::Repeat,
    tracking::{DayBlock, DayBlockSource, DayPlanLine, Item, Project, Session},
};
use salah_prayer::schedule;
use serde::Serialize;

/// The day in progress at `now`; after sleep it is already the coming day.
pub fn current(config: &Config, now: DateTime<Utc>) -> Result<NaiveDate, Error> {
    Ok(schedule::resolve(config, now)?.day())
}

/// Writes the block and plan snapshots of `day` when they are missing, and
/// refreshes the plan of `today` and later days from the current items.
pub fn materialize(
    tx: &WriteTx<'_>,
    config: &Config,
    day: NaiveDate,
    today: NaiveDate,
) -> Result<(), Error> {
    if tx.day_blocks(day)?.is_empty() {
        tx.replace_day_blocks(day, &scheduled_blocks(config, day)?)?;
    }
    let stored = tx.day_plan(day)?;
    if stored.is_empty() || day >= today {
        let planned = planned_lines(tx, day)?;
        if planned != stored {
            tx.replace_day_plan(day, &planned)?;
        }
    }
    Ok(())
}

/// The day's block snapshot, or the schedule's blocks when it has none.
pub fn blocks(store: &Store, config: &Config, day: NaiveDate) -> Result<Vec<DayBlock>, Error> {
    let stored = store.day_blocks(day)?;
    if stored.is_empty() {
        scheduled_blocks(config, day)
    } else {
        Ok(stored)
    }
}

/// The day's plan snapshot, or the current items' plan when it has none.
pub fn plan(store: &Store, day: NaiveDate) -> Result<Vec<DayPlanLine>, Error> {
    let stored = store.day_plan(day)?;
    if stored.is_empty() {
        planned_lines(store, day)
    } else {
        Ok(stored)
    }
}

fn scheduled_blocks(config: &Config, day: NaiveDate) -> Result<Vec<DayBlock>, Error> {
    let (_, source) = schedule::prayers_for(config, day)?;
    let source = if source.id == "official" {
        DayBlockSource::Official
    } else {
        DayBlockSource::Calculated
    };
    Ok(schedule::day_blocks(config, day)?
        .into_iter()
        .map(|window| DayBlock {
            day,
            block: window.key.start,
            starts_at: window.start,
            ends_at: window.end,
            source,
        })
        .collect())
}

fn planned_lines(store: &Store, day: NaiveDate) -> Result<Vec<DayPlanLine>, Error> {
    let mut lines: Vec<DayPlanLine> = items::list(store)?
        .into_iter()
        .filter(|item| item.applies_to(day))
        .map(|item| DayPlanLine {
            day,
            block: item.block,
            item_id: item.id,
            planned_minutes: item.planned_minutes,
        })
        .collect();
    lines.sort_by(|a, b| (a.block, &a.item_id).cmp(&(b.block, &b.item_id)));
    Ok(lines)
}

/// How much of a day's plan was done.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Fulfilment {
    pub planned_items: i64,
    pub done_items: i64,
    pub planned_minutes: i64,
    pub actual_minutes: i64,
}

impl Fulfilment {
    /// A plan line is done once a session of its item finished that day.
    pub fn of(plan: &[DayPlanLine], sessions: &[Session]) -> Self {
        Self {
            planned_items: plan.len() as i64,
            done_items: plan
                .iter()
                .filter(|line| has_session(sessions, &line.item_id))
                .count() as i64,
            planned_minutes: plan.iter().filter_map(|line| line.planned_minutes).sum(),
            actual_minutes: total_minutes(sessions.iter()),
        }
    }
}

/// Logged minutes; running sessions and ticks contribute nothing.
pub fn total_minutes<'a>(sessions: impl Iterator<Item = &'a Session>) -> i64 {
    sessions.filter_map(|s| s.minutes).sum()
}

fn has_session(sessions: &[Session], item_id: &str) -> bool {
    sessions
        .iter()
        .any(|s| !s.is_running() && s.item_id.as_deref() == Some(item_id))
}

/// The colour-coded project a row belongs to.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectRef {
    pub id: String,
    pub name: String,
    pub color: String,
}

impl From<&Project> for ProjectRef {
    fn from(project: &Project) -> Self {
        Self {
            id: project.id.clone(),
            name: project.name.clone(),
            color: project.color.clone(),
        }
    }
}

/// One planned item on one day.
#[derive(Clone, Debug, Serialize)]
pub struct DayRow {
    pub id: String,
    pub title: String,
    pub block: BoundaryId,
    pub repeat: Repeat,
    pub weekdays: Option<salah_domain::item::Weekdays>,
    pub position: i64,
    pub version: i64,
    pub project: Option<ProjectRef>,
    /// Minutes planned for this day (the snapshot of a past day).
    pub planned_minutes: Option<i64>,
    pub actual_minutes: i64,
    pub sessions: Vec<SessionView>,
    /// True once a session of the item finished that day (a tick counts).
    pub done: bool,
    pub schedule_text: String,
    pub next_occurrence: Option<NaiveDate>,
    /// `"20 / 30 min · Daily"`, `"20 min · Daily"`, or the schedule alone.
    pub detail_text: String,
}

/// A prayer block with its capacity and the work planned and done in it.
#[derive(Clone, Debug, Serialize)]
pub struct BlockPlan {
    pub start: BoundaryId,
    pub end: BoundaryId,
    pub capacity_minutes: i64,
    pub planned_minutes: i64,
    /// Minutes of every session recorded in the block, project-only ones too.
    pub actual_minutes: i64,
    pub items: Vec<DayRow>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NextBoundary {
    pub id: BoundaryId,
    pub name: &'static str,
    pub time: String,
    /// UTC instant of the boundary; clients derive countdowns from it.
    pub at: DateTime<Utc>,
    pub remaining_seconds: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Now {
    pub block: Option<BoundaryId>,
    pub sleeping: bool,
    pub next: NextBoundary,
}

#[derive(Clone, Debug, Serialize)]
pub struct PrayerRow {
    pub id: BoundaryId,
    pub name: &'static str,
    pub time: String,
    pub passed: bool,
    pub current: bool,
    pub marked: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SleepRow {
    pub time: String,
    pub passed: bool,
}

/// Everything the timeline of one day shows.
#[derive(Clone, Debug, Serialize)]
pub struct DayPlan {
    pub day: NaiveDate,
    pub is_today: bool,
    pub now: Option<Now>,
    pub prayers: Vec<PrayerRow>,
    pub sunrise: Option<String>,
    /// When the night block starts: the previous evening's sleep time.
    pub night_start: String,
    pub sleep: SleepRow,
    pub blocks: Vec<BlockPlan>,
    /// Planned items not done yet.
    pub open: usize,
    pub fulfilment: Fulfilment,
}

/// The widget's view of the day in progress.
#[derive(Clone, Debug, Serialize)]
pub struct Today {
    pub day: NaiveDate,
    pub block: BoundaryId,
    /// Where an item added now is placed.
    pub planning_block: BlockKey,
    /// Rows of the block in progress.
    pub current: Vec<DayRow>,
    /// Rows of `current` not done yet.
    pub open: usize,
    /// Every row planned today.
    pub items: Vec<DayRow>,
    pub prayer_marks: Vec<BoundaryId>,
    pub fulfilment: Fulfilment,
}

/// The day plan of `date` (default: the day in progress).
pub fn day_plan(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    date: Option<NaiveDate>,
) -> Result<DayPlan, Error> {
    let schedule = schedule::resolve(config, now)?;
    let today = schedule.day();
    let day = date.unwrap_or(today);
    let is_today = day == today;
    let current = is_today.then(|| schedule.current_block().start);
    let ledger = Ledger::load(store, config, day, today)?;

    let (prayer_times, _) = schedule::prayers_for(config, day)?;
    let time_of = |id: &str| {
        prayer_times
            .iter()
            .find(|p| p.id == id)
            .map(|p| format_minutes(p.minutes))
    };
    let marks = live_marks(store, day)?;
    let prayers = ledger
        .blocks
        .iter()
        .filter(|block| block.block.is_prayer())
        .map(|block| PrayerRow {
            id: block.block,
            name: block.block.name(),
            time: time_of(block.block.as_str()).unwrap_or_default(),
            passed: day < today || (is_today && block.starts_at <= now),
            current: current == Some(block.block),
            marked: marks.contains(&block.block),
        })
        .collect();
    let sleep_at = ledger.blocks.last().map(|block| block.ends_at);
    let blocks: Vec<BlockPlan> = BoundaryId::AWAKE
        .into_iter()
        .map(|start| ledger.block_plan(start))
        .collect();
    let open = blocks
        .iter()
        .flat_map(|block| &block.items)
        .filter(|row| !row.done)
        .count();
    let next = &schedule.timeline.next;
    Ok(DayPlan {
        day,
        is_today,
        now: is_today
            .then(|| -> Result<Now, Error> {
                Ok(Now {
                    block: current,
                    sleeping: schedule.timeline.sleeping(),
                    next: NextBoundary {
                        id: next.id,
                        name: next.name(),
                        time: format_minutes(next.minutes),
                        at: schedule::boundary_at(config, schedule.date, next)?,
                        remaining_seconds: schedule.timeline.remaining_seconds,
                    },
                })
            })
            .transpose()?,
        prayers,
        sunrise: time_of("sunrise"),
        night_start: config.sleep_time.clone(),
        sleep: SleepRow {
            time: config.sleep_time.clone(),
            passed: day < today || (is_today && sleep_at.is_some_and(|at| at <= now)),
        },
        blocks,
        open,
        fulfilment: ledger.fulfilment(),
    })
}

/// The widget's projection of the day in progress.
pub fn today(store: &mut Store, config: &Config, now: DateTime<Utc>) -> Result<Today, Error> {
    let schedule = schedule::resolve(config, now)?;
    let planning_block = schedule.planning_block();
    let current = schedule.current_block();
    let ledger = Ledger::load(store, config, current.day, schedule.day())?;
    let items: Vec<DayRow> = BoundaryId::AWAKE
        .into_iter()
        .flat_map(|block| ledger.rows(block))
        .collect();
    let current_rows: Vec<DayRow> = items
        .iter()
        .filter(|row| row.block == current.start)
        .cloned()
        .collect();
    Ok(Today {
        day: current.day,
        block: current.start,
        planning_block,
        open: current_rows.iter().filter(|row| !row.done).count(),
        current: current_rows,
        items,
        prayer_marks: live_marks(store, current.day)?,
        fulfilment: ledger.fulfilment(),
    })
}

/// The snapshots, items, projects, and sessions of one day.
struct Ledger {
    day: NaiveDate,
    blocks: Vec<DayBlock>,
    plan: Vec<DayPlanLine>,
    items: Vec<Item>,
    projects: Vec<Project>,
    sessions: Vec<Session>,
}

impl Ledger {
    fn load(
        store: &mut Store,
        config: &Config,
        day: NaiveDate,
        today: NaiveDate,
    ) -> Result<Self, Error> {
        store.write(|tx| materialize(tx, config, day, today))?;
        Ok(Self {
            day,
            blocks: store.day_blocks(day)?,
            plan: store.day_plan(day)?,
            items: store.items()?,
            projects: store.projects()?,
            sessions: store
                .sessions_between(day, day)?
                .into_iter()
                .filter(Session::is_live)
                .collect(),
        })
    }

    fn fulfilment(&self) -> Fulfilment {
        Fulfilment::of(&self.plan, &self.sessions)
    }

    fn block_plan(&self, start: BoundaryId) -> BlockPlan {
        let items = self.rows(start);
        BlockPlan {
            start,
            end: start.next().unwrap_or(BoundaryId::Sleep),
            capacity_minutes: self
                .blocks
                .iter()
                .filter(|block| block.block == start)
                .map(|block| (block.ends_at - block.starts_at).num_minutes().max(0))
                .sum(),
            planned_minutes: items.iter().filter_map(|row| row.planned_minutes).sum(),
            actual_minutes: total_minutes(self.sessions.iter().filter(|s| s.block.start == start)),
            items,
        }
    }

    /// The rows planned in `block`, in item position order.
    fn rows(&self, block: BoundaryId) -> Vec<DayRow> {
        let mut rows: Vec<DayRow> = self
            .plan
            .iter()
            .filter(|line| line.block == block)
            .filter_map(|line| {
                let item = self.items.iter().find(|item| item.id == line.item_id)?;
                Some(self.row(item, line))
            })
            .collect();
        rows.sort_by_key(|row| row.position);
        rows
    }

    fn row(&self, item: &Item, line: &DayPlanLine) -> DayRow {
        let sessions: Vec<&Session> = self
            .sessions
            .iter()
            .filter(|s| s.item_id.as_deref() == Some(item.id.as_str()))
            .collect();
        let actual_minutes = total_minutes(sessions.iter().copied());
        let schedule_text = item.schedule_text();
        let detail_text = match (actual_minutes, line.planned_minutes) {
            (actual, Some(planned)) => format!("{actual} / {planned} min · {schedule_text}"),
            (0, None) => schedule_text.clone(),
            (actual, None) => format!("{actual} min · {schedule_text}"),
        };
        DayRow {
            id: item.id.clone(),
            title: item.title.clone(),
            block: line.block,
            repeat: item.repeat,
            weekdays: item.weekdays,
            position: item.position,
            version: item.version,
            project: item
                .project_id
                .as_ref()
                .and_then(|id| self.projects.iter().find(|p| p.id == *id))
                .map(ProjectRef::from),
            planned_minutes: line.planned_minutes,
            actual_minutes,
            done: has_session(&self.sessions, &item.id),
            sessions: sessions.into_iter().map(SessionView::from).collect(),
            next_occurrence: item.next_occurrence(self.day),
            schedule_text,
            detail_text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{ItemChanges, NewItem};
    use crate::sessions::{self, NewLog, Target, Values};
    use crate::test_support::{at, config, date, ids, project};

    fn add(
        store: &mut Store,
        title: &str,
        block: BoundaryId,
        planned: Option<i64>,
        project_id: Option<&str>,
    ) -> Item {
        store
            .write(|tx| {
                items::add(
                    tx,
                    &config(),
                    at(20, 6, 0),
                    ids(),
                    NewItem {
                        title: title.into(),
                        block: Some(block),
                        repeat: Some(Repeat::Daily),
                        planned_minutes: planned,
                        project_id: project_id.map(str::to_owned),
                        ..NewItem::default()
                    },
                )
            })
            .unwrap()
    }

    fn log(store: &mut Store, item: &Item, minutes: i64, when: DateTime<Utc>) {
        store
            .write(|tx| {
                sessions::log(
                    tx,
                    &config(),
                    when,
                    ids(),
                    NewLog {
                        target: Target {
                            item_id: Some(item.id.clone()),
                            project_id: None,
                        },
                        minutes: Some(minutes),
                        values: Values::from([("focus".into(), 4.0), ("quality".into(), 4.0)]),
                        ..NewLog::default()
                    },
                )
            })
            .unwrap();
    }

    #[test]
    fn rows_carry_project_plan_sessions_and_fulfilment() {
        let mut store = Store::open_in_memory().unwrap();
        let quran = project(&mut store, "Quran");
        let read = add(
            &mut store,
            "Read",
            BoundaryId::Fajr,
            Some(30),
            Some(&quran.id),
        );
        add(&mut store, "Walk", BoundaryId::Asr, None, None);
        log(&mut store, &read, 20, at(20, 6, 30));

        let plan = day_plan(&mut store, &config(), at(20, 13, 0), None).unwrap();
        assert!(plan.is_today);
        assert_eq!(plan.now.as_ref().unwrap().block, Some(BoundaryId::Dhuhr));
        let fajr = &plan.blocks[1];
        assert_eq!(fajr.start, BoundaryId::Fajr);
        assert!(fajr.capacity_minutes > 0);
        assert_eq!((fajr.planned_minutes, fajr.actual_minutes), (30, 20));
        let row = &fajr.items[0];
        assert_eq!(row.project.as_ref().unwrap().name, "Quran");
        assert!(row.done && row.sessions.len() == 1);
        assert_eq!(row.detail_text, "20 / 30 min · Daily");
        assert_eq!(plan.open, 1);
        assert_eq!(
            plan.fulfilment,
            Fulfilment {
                planned_items: 2,
                done_items: 1,
                planned_minutes: 30,
                actual_minutes: 20,
            }
        );
        let widget = today(&mut store, &config(), at(20, 13, 0)).unwrap();
        assert_eq!((widget.block, widget.items.len()), (BoundaryId::Dhuhr, 2));
        assert!(widget.current.is_empty());
    }

    #[test]
    fn past_plans_keep_their_snapshot_while_today_follows_edits() {
        let mut store = Store::open_in_memory().unwrap();
        let read = add(&mut store, "Read", BoundaryId::Fajr, Some(30), None);
        day_plan(&mut store, &config(), at(20, 13, 0), None).unwrap();
        let changes = ItemChanges {
            planned_minutes: Some(Some(45)),
            ..ItemChanges::default()
        };
        store
            .write(|tx| items::edit(tx, at(21, 9, 0), &read.id, changes.clone(), None))
            .unwrap();
        let past = day_plan(&mut store, &config(), at(21, 9, 0), Some(date(20))).unwrap();
        let today = day_plan(&mut store, &config(), at(21, 9, 0), None).unwrap();
        assert_eq!(past.blocks[1].items[0].planned_minutes, Some(30));
        assert_eq!(today.blocks[1].items[0].planned_minutes, Some(45));
        assert!(past.now.is_none() && past.sleep.passed);
    }
}
