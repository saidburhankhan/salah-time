//! Planner use cases: the only place that combines the schedule, the item
//! rules, and the store. The CLI, the widget, and the desktop app all go
//! through these functions, so behaviour is identical everywhere.

mod entries;
mod items;
mod marks;
mod stats;
mod timers;
mod views;

pub use entries::{adjust_time, done, log, undo};
pub use items::{ItemChanges, NewItem, add, edit, find_item, move_item, remove};
pub use marks::{mark, unmark};
pub use stats::{DayStat, DayTotals, ItemStats, PrayerStats, Stats, Totals, stats};
pub use timers::{close_expired_timers, start_timer, stop_item_timer, stop_timer};
pub use views::{
    BlockPlan, DayPlan, DayReport, NextBoundary, Now, PrayerRow, RunningTimer, SleepRow, Today,
    day_plan, history, today,
};

use crate::config::Config;
use crate::item::{self, DayItem, DayView, Entry, Item, Repeat};
use crate::schedule::{self, BlockKey, Schedule};
use chrono::{DateTime, NaiveDate, SubsecRound, Utc};
use std::collections::HashMap;
use std::collections::hash_map::Entry as Slot;

/// The longest range `history` returns, to keep responses bounded.
pub const MAX_HISTORY_DAYS: i64 = 366;

/// Timestamps are stored with millisecond precision; truncate before use so a
/// value read back from the store equals the value that was written.
fn stamp(now: DateTime<Utc>) -> DateTime<Utc> {
    now.trunc_subsecs(3)
}

/// Caches the end instant of each block, computing a day's blocks once.
struct BlockEnds<'a> {
    config: &'a Config,
    days: HashMap<NaiveDate, Vec<schedule::BlockWindow>>,
}

impl<'a> BlockEnds<'a> {
    fn new(config: &'a Config) -> Self {
        Self {
            config,
            days: HashMap::new(),
        }
    }

    fn end(&mut self, key: BlockKey) -> Option<DateTime<Utc>> {
        let windows = match self.days.entry(key.day) {
            Slot::Occupied(slot) => slot.into_mut(),
            Slot::Vacant(slot) => slot.insert(schedule::day_blocks(self.config, key.day).ok()?),
        };
        windows.iter().find(|w| w.key == key).map(|w| w.end)
    }
}

/// The block that activity happening now is recorded in.
fn activity_block(schedule: &Schedule) -> BlockKey {
    schedule.current_block()
}

/// Where an entry for `item` made now belongs. A one-time item is always
/// recorded on its own day so a late tick still completes it.
fn entry_block(schedule: &Schedule, item: &Item) -> BlockKey {
    let now = activity_block(schedule);
    match item.repeat {
        Repeat::None if item.day != now.day => BlockKey {
            day: item.day,
            start: item.block,
        },
        _ => now,
    }
}

/// The block an entry recorded "on `date`" belongs to: the item's own block
/// that day. Without a date, the rules of `entry_block` apply.
fn recorded_block(schedule: &Schedule, item: &Item, date: Option<NaiveDate>) -> BlockKey {
    match date {
        Some(day) => BlockKey {
            day,
            start: item.block,
        },
        None => entry_block(schedule, item),
    }
}

fn report_day(
    items: &[Item],
    entries: &[Entry],
    ends: &mut BlockEnds,
    view: DayView,
) -> Vec<DayItem> {
    item::day_items(items, entries, view, |key| ends.end(key))
}

#[cfg(test)]
mod test_support {
    use super::*;
    use chrono::{FixedOffset, TimeZone};

    pub(super) fn config() -> Config {
        // No official cache for this method, so tests never read user data.
        Config {
            method: "mwl".into(),
            ..Config::default()
        }
    }

    /// Tashkent local time (UTC+5).
    pub(super) fn at(day: u32, h: u32, m: u32) -> DateTime<Utc> {
        FixedOffset::east_opt(5 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 11, day, h, m, 0)
            .unwrap()
            .with_timezone(&Utc)
    }

    pub(super) fn date(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
    }

    pub(super) fn titled(title: &str) -> NewItem {
        NewItem {
            title: title.into(),
            ..NewItem::default()
        }
    }

    /// A deterministic id source shared by planner tests.
    pub(super) fn ids() -> &'static crate::runtime::SequenceIds {
        static IDS: crate::runtime::SequenceIds = crate::runtime::SequenceIds::new();
        &IDS
    }
}
