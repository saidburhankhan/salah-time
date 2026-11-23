//! The day timeline and the widget's view of the day in progress.

use super::fulfilment::has_tick;
use super::{Fulfilment, Snapshot, block_minutes, freeze, snapshot, total_minutes};
use crate::Store;
use crate::config::Config;
use crate::marks::live_marks;
use crate::sessions::SessionView;
use crate::status::format_minutes;
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use salah_domain::{
    day::{BlockKey, BoundaryId},
    error::Error,
    item::{Repeat, Weekdays},
    tracking::{DayBlock, DayClock, DayPlanLine, Item, Project, Session},
};
use salah_prayer::schedule;
use serde::Serialize;

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
    pub weekdays: Option<Weekdays>,
    pub position: i64,
    pub version: i64,
    pub project: Option<ProjectRef>,
    /// The project and minutes as planned that day (the snapshot of a past day).
    pub planned_minutes: Option<i64>,
    pub actual_minutes: i64,
    pub sessions: Vec<SessionView>,
    /// True when the item has a tick on this day.
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
    /// Minutes of the block's sessions of planned items.
    pub actual_minutes: i64,
    /// Minutes of the block's other sessions: unplanned items, bare projects.
    pub unplanned_minutes: i64,
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

    let sunrise = ledger.clock.sunrise_minutes.map(format_minutes);
    let marks = live_marks(store, day)?;
    let zone = FixedOffset::east_opt(ledger.clock.offset_seconds)
        .ok_or_else(|| Error::Storage("invalid snapshot timezone offset".into()))?;
    let clock = |at: DateTime<Utc>| at.with_timezone(&zone).format("%H:%M").to_string();
    let prayers = ledger
        .blocks
        .iter()
        .filter(|block| block.block.is_prayer())
        .map(|block| PrayerRow {
            id: block.block,
            name: block.block.name(),
            time: clock(block.starts_at),
            passed: day < today || (is_today && block.starts_at <= now),
            current: current == Some(block.block),
            marked: marks.contains(&block.block),
        })
        .collect();
    let night_start = ledger.blocks.first().map(|block| block.starts_at);
    let sleep_at = ledger.clock.sleep_at;
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
        sunrise,
        night_start: night_start.map(clock).unwrap_or_default(),
        sleep: SleepRow {
            time: clock(sleep_at),
            passed: day < today || (is_today && sleep_at <= now),
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
    clock: DayClock,
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
        if day < today {
            store.write(|tx| freeze(tx, config, day, today))?;
        }
        let Snapshot {
            blocks,
            clock,
            plan,
        } = snapshot(store, config, day, today)?;
        Ok(Self {
            day,
            clock,
            blocks,
            plan,
            items: if day < today {
                store.items_on(day)?
            } else {
                store.items()?
            },
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
        let inside: Vec<Session> = self
            .sessions
            .iter()
            .filter(|s| s.block.start == start)
            .cloned()
            .collect();
        let fulfilment = Fulfilment::of(&self.plan, &inside);
        BlockPlan {
            start,
            end: start.next().unwrap_or(BoundaryId::Sleep),
            capacity_minutes: self
                .blocks
                .iter()
                .filter(|block| block.block == start)
                .map(block_minutes)
                .sum(),
            planned_minutes: items.iter().filter_map(|row| row.planned_minutes).sum(),
            actual_minutes: fulfilment.actual_minutes,
            unplanned_minutes: fulfilment.unplanned_minutes,
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
            project: line
                .project_id
                .as_ref()
                .and_then(|id| self.projects.iter().find(|p| p.id == *id))
                .map(ProjectRef::from),
            planned_minutes: line.planned_minutes,
            actual_minutes,
            done: has_tick(&self.sessions, &item.id),
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
    use crate::items::{self, ItemChanges, NewItem};
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
        assert!(!row.done);
        assert_eq!(row.sessions.len(), 1);
        assert_eq!(row.detail_text, "20 / 30 min · Daily");
        assert_eq!(plan.open, 2);
        assert_eq!(
            plan.fulfilment,
            Fulfilment {
                planned_items: 2,
                done_items: 0,
                planned_minutes: 30,
                actual_minutes: 20,
                unplanned_minutes: 0,
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
            .write(|tx| items::edit(tx, &config(), at(21, 9, 0), &read.id, changes.clone(), None))
            .unwrap();
        let past = day_plan(&mut store, &config(), at(21, 9, 0), Some(date(20))).unwrap();
        let today = day_plan(&mut store, &config(), at(21, 9, 0), None).unwrap();
        assert_eq!(past.blocks[1].items[0].planned_minutes, Some(30));
        assert_eq!(today.blocks[1].items[0].planned_minutes, Some(45));
        assert!(past.now.is_none() && past.sleep.passed);
    }

    fn project_log(store: &mut Store, project_id: &str, minutes: i64, when: DateTime<Utc>) {
        store
            .write(|tx| {
                sessions::log(
                    tx,
                    &config(),
                    when,
                    ids(),
                    NewLog {
                        target: Target {
                            item_id: None,
                            project_id: Some(project_id.into()),
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
    fn sessions_outside_the_plan_are_unplanned_minutes() {
        let mut store = Store::open_in_memory().unwrap();
        let quran = project(&mut store, "Quran");
        let read = add(
            &mut store,
            "Read",
            BoundaryId::Fajr,
            Some(30),
            Some(&quran.id),
        );
        log(&mut store, &read, 20, at(20, 6, 30));
        project_log(&mut store, &quran.id, 10, at(20, 6, 40));
        let plan = day_plan(&mut store, &config(), at(20, 13, 0), None).unwrap();
        assert_eq!(
            (
                plan.fulfilment.actual_minutes,
                plan.fulfilment.unplanned_minutes
            ),
            (20, 10)
        );
        let fajr = &plan.blocks[1];
        assert_eq!((fajr.actual_minutes, fajr.unplanned_minutes), (20, 10));
        assert_eq!(plan.fulfilment.ratio(), Some(20.0 / 30.0));
    }

    #[test]
    fn a_past_day_is_built_once_from_the_items_that_existed_then() {
        let mut store = Store::open_in_memory().unwrap();
        add(&mut store, "Read", BoundaryId::Fajr, Some(30), None);
        // Nothing read day 20; an item added on day 22 never existed on day 20,
        // and the empty plan written for day 21 is not rebuilt later.
        let later = store
            .write(|tx| {
                items::add(
                    tx,
                    &config(),
                    at(22, 9, 0),
                    ids(),
                    NewItem {
                        title: "Later".into(),
                        repeat: Some(Repeat::Daily),
                        date: Some(date(22)),
                        ..NewItem::default()
                    },
                )
            })
            .unwrap();
        assert!(store.day_written(date(20)).unwrap() && store.day_written(date(21)).unwrap());
        assert!(
            !store.day_written(date(22)).unwrap(),
            "today is never stored"
        );
        let past = day_plan(&mut store, &config(), at(22, 9, 0), Some(date(20))).unwrap();
        let rows: Vec<_> = past.blocks.iter().flat_map(|b| &b.items).collect();
        assert_eq!(rows.len(), 1);
        assert!(rows.iter().all(|row| row.id != later.id));
    }

    #[test]
    fn sparse_reads_and_long_gaps_preserve_the_plan_and_row_at_day_end() {
        let mut store = Store::open_in_memory().unwrap();
        let book = project(&mut store, "Book");
        let read = add(
            &mut store,
            "Read",
            BoundaryId::Fajr,
            Some(30),
            Some(&book.id),
        );
        let later = at(20, 9, 0) + chrono::Duration::days(400);
        let yesterday = later.date_naive().pred_opt().unwrap();
        day_plan(&mut store, &config(), later, Some(yesterday)).unwrap();
        let changes = ItemChanges {
            title: Some("Walk".into()),
            block: Some(BoundaryId::Asr),
            project_id: Some(None),
            planned_minutes: Some(Some(15)),
            repeat: Some(Repeat::None),
            ..Default::default()
        };
        store
            .write(|tx| items::edit(tx, &config(), later, &read.id, changes.clone(), None))
            .unwrap();
        assert!(store.day_written(yesterday.pred_opt().unwrap()).unwrap());
        for day in [date(21), yesterday] {
            let past = day_plan(&mut store, &config(), later, Some(day)).unwrap();
            let row = &past.blocks[1].items[0];
            assert_eq!(row.title, "Read");
            assert_eq!(row.repeat, Repeat::Daily);
            assert_eq!(row.planned_minutes, Some(30));
            assert_eq!(row.project.as_ref().unwrap().id, book.id);
        }
    }

    #[test]
    fn an_empty_written_plan_stays_empty() {
        let mut store = Store::open_in_memory().unwrap();
        day_plan(&mut store, &config(), at(21, 9, 0), Some(date(20))).unwrap();
        assert!(store.day_written(date(20)).unwrap());
        add(&mut store, "Read", BoundaryId::Fajr, Some(30), None);
        let past = day_plan(&mut store, &config(), at(21, 9, 0), Some(date(20))).unwrap();
        assert_eq!(past.fulfilment.planned_items, 0);
    }

    #[test]
    fn past_prayer_times_come_from_the_stored_blocks_and_today_is_live() {
        let mut store = Store::open_in_memory().unwrap();
        let past = day_plan(&mut store, &config(), at(21, 9, 0), Some(date(20))).unwrap();
        let moved = Config {
            sleep_time: "22:00".into(),
            timezone_offset: -3.0,
            latitude: 39.65,
            longitude: 66.96,
            ..config()
        };
        let again = day_plan(&mut store, &moved, at(21, 9, 0), Some(date(20))).unwrap();
        assert_eq!(again.sunrise, past.sunrise);
        assert_eq!(again.night_start, past.night_start);
        assert_eq!(again.sleep.time, past.sleep.time);
        assert_eq!(again.prayers[0].time, past.prayers[0].time);
        let today = day_plan(&mut store, &moved, at(21, 9, 0), None).unwrap();
        assert_eq!(today.sleep.time, "22:00");
        assert_ne!(today.prayers[0].time, past.prayers[0].time);
    }

    #[test]
    fn day_capacity_leaves_out_the_night() {
        let mut store = Store::open_in_memory().unwrap();
        let plan = day_plan(&mut store, &config(), at(20, 13, 0), None).unwrap();
        let awake: i64 = plan.blocks[1..].iter().map(|b| b.capacity_minutes).sum();
        let blocks = super::super::snapshot(&store, &config(), date(20), date(20))
            .unwrap()
            .blocks;
        assert_eq!(super::super::day_capacity(&blocks), awake);
        assert!(awake < 1440);
    }
}
