use super::marks::live_marks;
use super::{BlockEnds, MAX_HISTORY_DAYS, report_day, stamp};
use crate::config::Config;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use salah_domain::day::BoundaryId;
use salah_domain::error::Error;
use salah_domain::item::{self, DayItem, DayView, State};
use salah_prayer::schedule;
use salah_store::Store;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct DayReport {
    pub day: NaiveDate,
    pub items: Vec<DayItem>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Today {
    pub day: NaiveDate,
    /// The block in progress (`night` between sleep and Fajr). Always set;
    /// optional only for compatibility with older readers.
    pub block: Option<BoundaryId>,
    /// Where `add` places a new item right now.
    pub planning_block: schedule::BlockKey,
    /// Items for the block in progress, carried-forward ones included.
    pub current: Vec<DayItem>,
    /// Open items in `current`.
    pub open: usize,
    /// Every item planned today.
    pub items: Vec<DayItem>,
    /// Always empty: the night now belongs to the coming day. Kept for
    /// compatibility with older readers.
    pub upcoming: Vec<DayItem>,
    /// Prayers marked as prayed on `day`.
    pub prayer_marks: Vec<BoundaryId>,
    /// The running timer, if any.
    pub timer: Option<RunningTimer>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RunningTimer {
    pub item_id: String,
    pub title: String,
    pub started_at: DateTime<Utc>,
    /// Minutes the item has today, the running session included.
    pub minutes: i64,
}

pub fn today(store: &Store, config: &Config, now: DateTime<Utc>) -> Result<Today, Error> {
    let now = stamp(now);
    let schedule = schedule::resolve(config, now)?;
    let current_block = schedule.current_block();
    let day = current_block.day;
    let block = current_block.start;
    let items = store.items()?;
    let entries = store.entries_between(day, day)?;
    let mut ends = BlockEnds::new(config);
    let rows = report_day(
        &items,
        &entries,
        &mut ends,
        DayView {
            day,
            current: Some(block),
            ended: false,
            now,
        },
    );
    let current = item::current_block_items(&rows, block);
    let open = current.iter().filter(|r| r.state == State::Open).count();
    let prayer_marks = live_marks(store, day)?;
    let timer = rows.iter().find_map(|row| {
        row.timer_started_at.map(|started_at| RunningTimer {
            item_id: row.id.clone(),
            title: row.title.clone(),
            started_at,
            minutes: row.minutes,
        })
    });
    Ok(Today {
        day,
        block: Some(block),
        planning_block: current_block,
        current,
        open,
        items: rows,
        upcoming: Vec::new(),
        prayer_marks,
        timer,
    })
}

/// Every day in the inclusive range with its items and their outcome.
pub fn history(
    store: &Store,
    config: &Config,
    now: DateTime<Utc>,
    first: NaiveDate,
    last: NaiveDate,
) -> Result<Vec<DayReport>, Error> {
    if last < first {
        return Err("history range ends before it starts".into());
    }
    if (last - first).num_days() >= MAX_HISTORY_DAYS {
        return Err(format!("history covers at most {MAX_HISTORY_DAYS} days").into());
    }
    let now = stamp(now);
    let schedule = schedule::resolve(config, now)?;
    let today = schedule.day();
    let current = schedule.current_block().start;
    let items = store.items()?;
    let entries = store.entries_between(first, last)?;
    let mut ends = BlockEnds::new(config);
    let mut days = Vec::new();
    let mut day = first;
    while day <= last {
        let view = DayView {
            day,
            current: (day == today).then_some(current),
            // A day ends at sleep, after which the next day is today.
            ended: day < today,
            now,
        };
        days.push(DayReport {
            day,
            items: report_day(&items, &entries, &mut ends, view),
        });
        day += Duration::days(1);
    }
    Ok(days)
}

#[derive(Clone, Debug, Serialize)]
pub struct NextBoundary {
    pub id: BoundaryId,
    pub name: &'static str,
    pub time: String,
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

#[derive(Clone, Debug, Serialize)]
pub struct BlockPlan {
    pub start: BoundaryId,
    pub end: BoundaryId,
    pub items: Vec<DayItem>,
}

/// Everything the timeline view of one day needs.
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
    pub open: usize,
}

pub fn day_plan(
    store: &Store,
    config: &Config,
    now: DateTime<Utc>,
    date: Option<NaiveDate>,
) -> Result<DayPlan, Error> {
    let now = stamp(now);
    let schedule = schedule::resolve(config, now)?;
    let today = schedule.day();
    let day = date.unwrap_or(today);
    let is_today = day == today;
    let sleeping = schedule.timeline.sleeping();
    let current = is_today.then(|| schedule.current_block().start);
    let ended = day < today;
    let (prayers, _) = schedule::prayers_for(config, day)?;
    let windows = schedule::day_blocks(config, day)?;
    let marks = live_marks(store, day)?;
    let time_of = |id: &str| {
        prayers
            .iter()
            .find(|p| p.id == id)
            .map(|p| crate::status::format_minutes(p.minutes))
    };
    let rows = windows
        .iter()
        .filter(|window| window.key.start.is_prayer())
        .map(|window| {
            let id = window.key.start;
            PrayerRow {
                id,
                name: id.name(),
                time: time_of(id.as_str()).unwrap_or_default(),
                passed: day < today || (is_today && window.start <= now),
                current: current == Some(id),
                marked: marks.contains(&id),
            }
        })
        .collect();
    let sleep_end = windows.last().map(|w| w.end);
    let items = store.items()?;
    let entries = store.entries_between(day, day)?;
    let mut ends = BlockEnds::new(config);
    let day_rows = report_day(
        &items,
        &entries,
        &mut ends,
        DayView {
            day,
            current,
            ended,
            now,
        },
    );
    let open = day_rows.iter().filter(|r| r.state == State::Open).count();
    let blocks = BoundaryId::AWAKE
        .into_iter()
        .map(|start| BlockPlan {
            start,
            end: start.next().unwrap_or(BoundaryId::Sleep),
            items: day_rows
                .iter()
                .filter(|r| r.block == start)
                .cloned()
                .collect(),
        })
        .collect();
    let next = &schedule.timeline.next;
    Ok(DayPlan {
        day,
        is_today,
        now: is_today.then(|| Now {
            block: current,
            sleeping,
            next: NextBoundary {
                id: next.id,
                name: next.name(),
                time: crate::status::format_minutes(next.minutes),
                remaining_seconds: schedule.timeline.remaining_seconds,
            },
        }),
        prayers: rows,
        sunrise: time_of("sunrise"),
        night_start: config.sleep_time.clone(),
        sleep: SleepRow {
            time: config.sleep_time.clone(),
            passed: day < today || (is_today && sleep_end.is_some_and(|end| end <= now)),
        },
        blocks,
        open,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::add;
    use crate::planner::test_support::{at, config, date, ids, titled};

    #[test]
    fn open_items_carry_forward_and_are_missed_once_the_day_ends() {
        let mut store = Store::open_in_memory().unwrap();
        let morning = add(&mut store, &config(), at(20, 9, 0), ids(), titled("Email")).unwrap();
        let later = today(&store, &config(), at(20, 16, 0)).unwrap();
        assert_eq!(later.current[0].id, morning.id, "carried into Asr");
        let past = history(&store, &config(), at(21, 9, 0), date(20), date(20)).unwrap();
        assert_eq!(past[0].items[0].state, State::Missed);
    }

    #[test]
    fn day_plan_describes_the_timeline() {
        let mut store = Store::open_in_memory().unwrap();
        add(&mut store, &config(), at(20, 13, 0), ids(), titled("Email")).unwrap();
        let plan = day_plan(&store, &config(), at(20, 13, 0), None).unwrap();
        assert!(plan.is_today);
        let now = plan.now.as_ref().unwrap();
        assert_eq!(
            (now.block, now.next.id),
            (Some(BoundaryId::Dhuhr), BoundaryId::Asr)
        );
        assert_eq!(plan.prayers.len(), 5);
        assert!(plan.prayers[0].passed && plan.prayers[1].current && !plan.prayers[2].passed);
        assert_eq!(plan.blocks.len(), 6);
        assert_eq!(plan.blocks[2].items[0].title, "Email");
        assert_eq!(plan.blocks[5].end, BoundaryId::Sleep);
        assert_eq!(plan.open, 1);
        let past = day_plan(&store, &config(), at(20, 13, 0), Some(date(18))).unwrap();
        assert!(past.now.is_none() && past.prayers.iter().all(|p| p.passed) && past.sleep.passed);
        assert!(past.blocks.iter().all(|b| b.items.is_empty()));
    }
}
