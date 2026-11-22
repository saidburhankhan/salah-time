//! Planner use cases: the only place that combines the schedule, the item
//! rules, and the store. The CLI, the widget, and the desktop app all go
//! through these functions, so behaviour is identical everywhere.

use crate::config::Config;
use crate::day::BoundaryId;
use crate::item::{
    self, DayItem, DayView, Entry, EntryKind, Item, Measure, PrayerMark, Repeat, State, Weekdays,
    validate_block, validate_repeat, validate_title,
};
use crate::schedule::{self, BlockKey, Schedule};
use crate::store::Store;
use chrono::{DateTime, Duration, NaiveDate, SubsecRound, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::collections::hash_map::Entry as Slot;

/// The longest range `history` returns, to keep responses bounded.
pub const MAX_HISTORY_DAYS: i64 = 366;
const MIN_ID_PREFIX: usize = 4;

/// Timestamps are stored with millisecond precision; truncate before use so a
/// value read back from the store equals the value that was written.
fn stamp(now: DateTime<Utc>) -> DateTime<Utc> {
    now.trunc_subsecs(3)
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
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

/// The next free position at the end of a block.
fn next_position(store: &Store, block: BoundaryId) -> Result<i64, String> {
    Ok(store
        .items()?
        .iter()
        .filter(|i| i.is_live() && i.block == block)
        .map(|i| i.position + 1)
        .max()
        .unwrap_or(0))
}

/// Persists the automatic stop of timers whose block has ended.
pub fn close_expired_timers(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
) -> Result<usize, String> {
    let now = stamp(now);
    let mut ends = BlockEnds::new(config);
    let expired: Vec<(Entry, DateTime<Utc>)> = store
        .running_timers()?
        .into_iter()
        .filter_map(|timer| {
            let end = ends.end(timer.block)?;
            (end <= now).then_some((timer, end))
        })
        .collect();
    store.transaction(|store| {
        for (mut timer, end) in expired.iter().cloned() {
            timer.ended_at = Some(end.max(timer.started_at.unwrap_or(end)));
            timer.updated_at = now;
            store.put_entry(&timer)?;
        }
        Ok(expired.len())
    })
}

/// Finds a live item by its full id or a unique prefix of at least four
/// characters.
pub fn find_item(store: &Store, id: &str) -> Result<Item, String> {
    let id = id.trim();
    if let Some(item) = store.item(id)?.filter(Item::is_live) {
        return Ok(item);
    }
    if id.len() < MIN_ID_PREFIX {
        return Err(format!(
            "no item {id:?}; use at least {MIN_ID_PREFIX} characters of the id"
        ));
    }
    let mut matches = store
        .items()?
        .into_iter()
        .filter(|item| item.is_live() && item.id.starts_with(id));
    match (matches.next(), matches.next()) {
        (Some(item), None) => Ok(item),
        (None, _) => Err(format!("no item {id:?}")),
        (Some(_), Some(_)) => Err(format!("item id {id:?} is ambiguous; use more characters")),
    }
}

#[derive(Clone, Debug, Default)]
pub struct NewItem {
    pub title: String,
    /// Defaults to the block being planned now.
    pub block: Option<BoundaryId>,
    pub repeat: Option<Repeat>,
    pub weekdays: Option<Weekdays>,
    pub measure: Option<Measure>,
    pub target: Option<i64>,
    /// Day to plan on (one-time) or start from (repeating). Defaults to the
    /// day being planned now.
    pub date: Option<NaiveDate>,
}

pub fn add(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    request: NewItem,
) -> Result<Item, String> {
    let now = stamp(now);
    let schedule = schedule::resolve(config, now)?;
    let planning = schedule.planning_block();
    let measure = request.measure.unwrap_or(Measure::Check);
    if let Some(target) = request.target {
        if measure == Measure::Check {
            return Err("a target needs --measure minutes or count".into());
        }
        if target <= 0 {
            return Err("target must be positive".into());
        }
    }
    let repeat = request.repeat.unwrap_or(Repeat::None);
    validate_repeat(repeat, request.weekdays)?;
    let day = request.date.unwrap_or(planning.day);
    // Another day has no "current" block, so default to its first block.
    let default_block = if day == planning.day {
        planning.start
    } else {
        BoundaryId::Fajr
    };
    let block = validate_block(request.block.unwrap_or(default_block))?;
    let item = Item {
        id: new_id(),
        title: validate_title(&request.title)?,
        block,
        repeat,
        weekdays: request.weekdays,
        measure,
        target: request.target,
        position: next_position(store, block)?,
        day,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    store.put_item(&item)?;
    Ok(item)
}

#[derive(Clone, Debug, Default)]
pub struct ItemChanges {
    pub title: Option<String>,
    pub block: Option<BoundaryId>,
    pub target: Option<Option<i64>>,
    pub repeat: Option<Repeat>,
    pub weekdays: Option<Weekdays>,
    pub measure: Option<Measure>,
}

pub fn edit(
    store: &mut Store,
    now: DateTime<Utc>,
    id: &str,
    changes: ItemChanges,
) -> Result<Item, String> {
    let mut item = find_item(store, id)?;
    if let Some(title) = changes.title {
        item.title = validate_title(&title)?;
    }
    if let Some(block) = changes.block {
        item.block = validate_block(block)?;
    }
    if let Some(repeat) = changes.repeat {
        item.repeat = repeat;
        if repeat != Repeat::Weekly {
            item.weekdays = None;
        }
    }
    if let Some(weekdays) = changes.weekdays {
        item.weekdays = Some(weekdays);
    }
    validate_repeat(item.repeat, item.weekdays)?;
    if let Some(measure) = changes.measure {
        item.measure = measure;
        if measure == Measure::Check {
            item.target = None;
        }
    }
    if let Some(target) = changes.target {
        if target.is_some_and(|t| t <= 0) {
            return Err("target must be positive".into());
        }
        if target.is_some() && item.measure == Measure::Check {
            return Err("a target needs a minutes or count item".into());
        }
        item.target = target;
    }
    item.updated_at = stamp(now);
    store.put_item(&item)?;
    Ok(item)
}

/// Moves an item to `block` at `index` (0-based) and renumbers that block.
pub fn move_item(
    store: &mut Store,
    now: DateTime<Utc>,
    id: &str,
    block: BoundaryId,
    index: usize,
) -> Result<Item, String> {
    let now = stamp(now);
    let mut moved = find_item(store, id)?;
    let block = validate_block(block)?;
    let mut siblings: Vec<Item> = store
        .items()?
        .into_iter()
        .filter(|i| i.is_live() && i.block == block && i.id != moved.id)
        .collect();
    siblings.sort_by(|a, b| {
        a.position
            .cmp(&b.position)
            .then(a.created_at.cmp(&b.created_at))
    });
    let moved_id = moved.id.clone();
    moved.block = block;
    siblings.insert(index.min(siblings.len()), moved);
    store.transaction(|store| {
        for (position, item) in siblings.iter_mut().enumerate() {
            let position = position as i64;
            if item.position != position || item.id == moved_id {
                item.position = position;
                item.updated_at = now;
                store.put_item(item)?;
            }
        }
        Ok(())
    })?;
    siblings
        .into_iter()
        .find(|i| i.id == moved_id)
        .ok_or_else(|| "moved item vanished".into())
}

pub fn remove(store: &mut Store, now: DateTime<Utc>, id: &str) -> Result<Item, String> {
    let mut item = find_item(store, id)?;
    let now = stamp(now);
    item.deleted_at = Some(now);
    item.updated_at = now;
    store.put_item(&item)?;
    Ok(item)
}

fn new_entry(item: &Item, block: BlockKey, kind: EntryKind, now: DateTime<Utc>) -> Entry {
    Entry {
        id: new_id(),
        item_id: item.id.clone(),
        block,
        kind,
        value: None,
        started_at: None,
        ended_at: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    }
}

/// Ticks an item off for its day.
pub fn done(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    date: Option<NaiveDate>,
) -> Result<Entry, String> {
    let now = stamp(now);
    let item = find_item(store, id)?;
    let schedule = schedule::resolve(config, now)?;
    let block = recorded_block(&schedule, &item, date);
    let entry = new_entry(&item, block, EntryKind::Done, now);
    store.put_entry(&entry)?;
    Ok(entry)
}

/// Removes the tick from an item's day. Logged minutes and counts stay.
pub fn undo(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    date: Option<NaiveDate>,
) -> Result<usize, String> {
    let now = stamp(now);
    let item = find_item(store, id)?;
    let schedule = schedule::resolve(config, now)?;
    let day = recorded_block(&schedule, &item, date).day;
    let ticks: Vec<Entry> = store
        .entries_between(day, day)?
        .into_iter()
        .filter(|e| e.is_live() && e.item_id == item.id && e.kind == EntryKind::Done)
        .collect();
    store.transaction(|store| {
        for mut tick in ticks.iter().cloned() {
            tick.deleted_at = Some(now);
            tick.updated_at = now;
            store.put_entry(&tick)?;
        }
        Ok(ticks.len())
    })
}

/// Records minutes or a count against an item, matching its measure.
pub fn log(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    value: i64,
    date: Option<NaiveDate>,
) -> Result<Entry, String> {
    let item = find_item(store, id)?;
    // Count items adjust their count; every other item records time spent.
    let kind = match item.measure {
        Measure::Count => EntryKind::Count,
        Measure::Check | Measure::Minutes => EntryKind::Minutes,
    };
    record(store, config, now, &item, kind, value, date)
}

/// Adds (or, when negative, subtracts) minutes of time spent on any item,
/// including count items, whose `log` adjusts the count instead.
pub fn adjust_time(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    value: i64,
    date: Option<NaiveDate>,
) -> Result<Entry, String> {
    let item = find_item(store, id)?;
    record(store, config, now, &item, EntryKind::Minutes, value, date)
}

fn record(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    item: &Item,
    kind: EntryKind,
    value: i64,
    date: Option<NaiveDate>,
) -> Result<Entry, String> {
    let now = stamp(now);
    // A negative value subtracts; daily totals are clamped at zero.
    if value == 0 || value.abs() > 24 * 60 {
        return Err("value must be between -1440 and 1440, and not 0".into());
    }
    let schedule = schedule::resolve(config, now)?;
    let mut entry = new_entry(item, recorded_block(&schedule, item, date), kind, now);
    entry.value = Some(value);
    store.put_entry(&entry)?;
    Ok(entry)
}

/// Starts a timer for a minutes item, stopping any other running timer.
pub fn start_timer(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
) -> Result<Entry, String> {
    let now = stamp(now);
    let item = find_item(store, id)?;
    let schedule = schedule::resolve(config, now)?;
    close_expired_timers(store, config, now)?;
    let mut timer = new_entry(&item, schedule.current_block(), EntryKind::Timer, now);
    timer.started_at = Some(now);
    store.transaction(|store| {
        stop_running(store, now)?;
        store.put_entry(&timer)?;
        Ok(())
    })?;
    Ok(timer)
}

fn stop_running(store: &Store, now: DateTime<Utc>) -> Result<Vec<Entry>, String> {
    let mut stopped = Vec::new();
    for mut timer in store.running_timers()? {
        timer.ended_at = Some(now);
        timer.updated_at = now;
        store.put_entry(&timer)?;
        stopped.push(timer);
    }
    Ok(stopped)
}

/// Stops `id`'s timer if it is running; other timers keep running.
pub fn stop_item_timer(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
) -> Result<Vec<Entry>, String> {
    let now = stamp(now);
    let item = find_item(store, id)?;
    close_expired_timers(store, config, now)?;
    store.transaction(|store| {
        let mut stopped = Vec::new();
        for mut timer in store.running_timers()? {
            if timer.item_id == item.id {
                timer.ended_at = Some(now);
                timer.updated_at = now;
                store.put_entry(&timer)?;
                stopped.push(timer);
            }
        }
        Ok(stopped)
    })
}

/// Stops the running timer, if any.
pub fn stop_timer(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
) -> Result<Vec<Entry>, String> {
    let now = stamp(now);
    close_expired_timers(store, config, now)?;
    store.transaction(|store| stop_running(store, now))
}

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
    pub planning_block: BlockKey,
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

fn report_day(
    items: &[Item],
    entries: &[Entry],
    ends: &mut BlockEnds,
    view: DayView,
) -> Vec<DayItem> {
    item::day_items(items, entries, view, |key| ends.end(key))
}

pub fn today(store: &Store, config: &Config, now: DateTime<Utc>) -> Result<Today, String> {
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
) -> Result<Vec<DayReport>, String> {
    if last < first {
        return Err("history range ends before it starts".into());
    }
    if (last - first).num_days() >= MAX_HISTORY_DAYS {
        return Err(format!("history covers at most {MAX_HISTORY_DAYS} days"));
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

fn live_marks(store: &Store, day: NaiveDate) -> Result<Vec<BoundaryId>, String> {
    let mut marks: Vec<BoundaryId> = store
        .marks_between(day, day)?
        .into_iter()
        .filter(PrayerMark::is_live)
        .map(|m| m.prayer)
        .collect();
    marks.sort();
    Ok(marks)
}

fn mark_prayer(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    prayer: BoundaryId,
    date: Option<NaiveDate>,
    prayed: bool,
) -> Result<PrayerMark, String> {
    if !prayer.is_prayer() {
        return Err(format!("{} is not a prayer", prayer.name()));
    }
    let now = stamp(now);
    let day = match date {
        Some(day) => day,
        None => schedule::resolve(config, now)?.day(),
    };
    let id = PrayerMark::id_for(day, prayer);
    let mut mark = store.mark(&id)?.unwrap_or(PrayerMark {
        id,
        day,
        prayer,
        marked_at: now,
        created_at: now,
        updated_at: now,
        deleted_at: Some(now),
    });
    // Repeating the current state changes nothing, so marking twice is safe.
    if mark.is_live() == prayed {
        return Ok(mark);
    }
    if prayed {
        mark.marked_at = now;
        mark.deleted_at = None;
    } else {
        mark.deleted_at = Some(now);
    }
    mark.updated_at = now;
    store.put_mark(&mark)?;
    Ok(mark)
}

/// Marks a prayer as prayed on `date` (default: the current day).
pub fn mark(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    prayer: BoundaryId,
    date: Option<NaiveDate>,
) -> Result<PrayerMark, String> {
    mark_prayer(store, config, now, prayer, date, true)
}

/// Removes a prayer's mark on `date` (default: the current day).
pub fn unmark(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    prayer: BoundaryId,
    date: Option<NaiveDate>,
) -> Result<PrayerMark, String> {
    mark_prayer(store, config, now, prayer, date, false)
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
) -> Result<DayPlan, String> {
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

#[derive(Clone, Debug, Serialize)]
pub struct DayStat {
    pub day: NaiveDate,
    pub planned: bool,
    pub done: bool,
    pub minutes: i64,
    pub count: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ItemStats {
    pub id: String,
    pub title: String,
    pub block: BoundaryId,
    pub repeat: Repeat,
    pub weekdays: Option<Weekdays>,
    pub measure: Measure,
    pub target: Option<i64>,
    pub planned_days: usize,
    pub done_days: usize,
    pub minutes: i64,
    pub count: i64,
    pub series: Vec<DayStat>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PrayerStats {
    pub prayer: BoundaryId,
    pub marked_days: usize,
    pub days: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Stats {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub days: usize,
    pub items: Vec<ItemStats>,
    pub prayers: Vec<PrayerStats>,
    pub marked_total: usize,
    pub prayer_slots: usize,
    /// One row per day in the range, oldest first.
    pub daily: Vec<DayTotals>,
    /// The range as a whole.
    pub totals: Totals,
}

/// Everything that happened on one day, summed over its items.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DayTotals {
    pub day: NaiveDate,
    /// Items planned that day.
    pub planned: usize,
    /// Planned items that were done.
    pub done: usize,
    /// Minutes spent on all items.
    pub minutes: i64,
    /// Counts recorded on counter items.
    pub count: i64,
    /// Prayers marked as prayed (0–5).
    pub prayers: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Totals {
    pub planned: usize,
    pub done: usize,
    pub minutes: i64,
    pub count: i64,
    pub prayers: usize,
    /// Days with at least one item done or any time spent.
    pub active_days: usize,
}

/// Per-item and per-prayer totals over an inclusive range of days.
pub fn stats(
    store: &Store,
    config: &Config,
    now: DateTime<Utc>,
    first: NaiveDate,
    last: NaiveDate,
) -> Result<Stats, String> {
    let reports = history(store, config, now, first, last)?;
    let days = reports.len();
    let mut items: Vec<ItemStats> = Vec::new();
    for item in store.items()?.into_iter().filter(Item::is_live) {
        let series: Vec<DayStat> = reports
            .iter()
            .map(|report| {
                let row = report.items.iter().find(|r| r.id == item.id);
                DayStat {
                    day: report.day,
                    planned: row.is_some(),
                    done: row.is_some_and(|r| r.state == State::Done),
                    minutes: row.map_or(0, |r| r.minutes),
                    count: row.map_or(0, |r| r.count),
                }
            })
            .collect();
        // Repeating items are always listed, even before their first
        // occurrence; one-time items only when planned in the range.
        if item.repeat == Repeat::None && !series.iter().any(|d| d.planned) {
            continue;
        }
        items.push(ItemStats {
            planned_days: series.iter().filter(|d| d.planned).count(),
            done_days: series.iter().filter(|d| d.done).count(),
            minutes: series.iter().map(|d| d.minutes).sum(),
            count: series.iter().map(|d| d.count).sum(),
            id: item.id,
            title: item.title,
            block: item.block,
            repeat: item.repeat,
            weekdays: item.weekdays,
            measure: item.measure,
            target: item.target,
            series,
        });
    }
    items.sort_by(|a, b| a.block.cmp(&b.block).then_with(|| a.title.cmp(&b.title)));
    let marks: Vec<PrayerMark> = store
        .marks_between(first, last)?
        .into_iter()
        .filter(PrayerMark::is_live)
        .collect();
    let prayers: Vec<PrayerStats> = BoundaryId::PRAYERS
        .into_iter()
        .map(|prayer| PrayerStats {
            prayer,
            marked_days: marks.iter().filter(|m| m.prayer == prayer).count(),
            days,
        })
        .collect();
    let daily: Vec<DayTotals> = reports
        .iter()
        .map(|report| DayTotals {
            day: report.day,
            planned: report.items.len(),
            done: report
                .items
                .iter()
                .filter(|r| r.state == State::Done)
                .count(),
            minutes: report.items.iter().map(|r| r.minutes).sum(),
            count: report.items.iter().map(|r| r.count).sum(),
            prayers: marks.iter().filter(|m| m.day == report.day).count(),
        })
        .collect();
    let totals = Totals {
        planned: daily.iter().map(|d| d.planned).sum(),
        done: daily.iter().map(|d| d.done).sum(),
        minutes: daily.iter().map(|d| d.minutes).sum(),
        count: daily.iter().map(|d| d.count).sum(),
        prayers: marks.len(),
        active_days: daily.iter().filter(|d| d.done > 0 || d.minutes > 0).count(),
    };
    Ok(Stats {
        from: first,
        to: last,
        days,
        daily,
        totals,
        marked_total: marks.len(),
        prayer_slots: days * BoundaryId::PRAYERS.len(),
        items,
        prayers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, TimeZone};

    fn config() -> Config {
        // No official cache for this method, so tests never read user data.
        Config {
            method: "mwl".into(),
            ..Config::default()
        }
    }

    /// Tashkent local time (UTC+5).
    fn at(day: u32, h: u32, m: u32) -> DateTime<Utc> {
        FixedOffset::east_opt(5 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 11, day, h, m, 0)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn date(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
    }

    fn titled(title: &str) -> NewItem {
        NewItem {
            title: title.into(),
            ..NewItem::default()
        }
    }

    #[test]
    fn add_defaults_to_the_current_block() {
        let mut store = Store::open_in_memory().unwrap();
        let item = add(
            &mut store,
            &config(),
            at(20, 13, 0),
            titled("Call the bank"),
        )
        .unwrap();
        assert_eq!(item.block, BoundaryId::Dhuhr);
        assert_eq!(item.day, date(20));
        let today = today(&store, &config(), at(20, 13, 5)).unwrap();
        assert_eq!(today.open, 1);
        assert_eq!(today.current[0].title, "Call the bank");
    }

    #[test]
    fn the_night_before_fajr_is_a_plannable_block_of_the_coming_day() {
        let mut store = Store::open_in_memory().unwrap();
        // 23:30 on the 20th is after the default 23:00 sleep: the 21st's night.
        let item = add(&mut store, &config(), at(20, 23, 30), titled("Tahajjud")).unwrap();
        assert_eq!((item.day, item.block), (date(21), BoundaryId::Night));
        let view = today(&store, &config(), at(21, 2, 0)).unwrap();
        assert_eq!((view.day, view.block), (date(21), Some(BoundaryId::Night)));
        assert_eq!(view.current[0].title, "Tahajjud");
        assert_eq!(view.open, 1);
        let timer = NewItem {
            measure: Some(Measure::Minutes),
            ..titled("Study")
        };
        let study = add(&mut store, &config(), at(21, 3, 0), timer).unwrap();
        start_timer(&mut store, &config(), at(21, 3, 0), &study.id).unwrap();
        let plan = day_plan(&store, &config(), at(21, 3, 30), None).unwrap();
        assert_eq!(plan.blocks[0].start, BoundaryId::Night);
        assert_eq!(plan.blocks[0].items.len(), 2);
        assert_eq!(plan.now.as_ref().unwrap().block, Some(BoundaryId::Night));
        assert!(mark(&mut store, &config(), at(21, 3, 0), BoundaryId::Night, None).is_err());
    }

    #[test]
    fn item_added_after_midnight_before_late_bedtime_belongs_to_the_previous_day() {
        let late = Config {
            sleep_time: "00:30".into(),
            ..config()
        };
        let mut store = Store::open_in_memory().unwrap();
        let item = add(&mut store, &late, at(21, 0, 10), titled("Review the day")).unwrap();
        assert_eq!((item.day, item.block), (date(20), BoundaryId::Isha));
    }

    #[test]
    fn open_items_carry_forward_and_are_missed_once_the_day_ends() {
        let mut store = Store::open_in_memory().unwrap();
        let morning = add(&mut store, &config(), at(20, 9, 0), titled("Email")).unwrap();
        let later = today(&store, &config(), at(20, 16, 0)).unwrap();
        assert_eq!(later.current[0].id, morning.id, "carried into Asr");
        let past = history(&store, &config(), at(21, 9, 0), date(20), date(20)).unwrap();
        assert_eq!(past[0].items[0].state, State::Missed);
    }

    #[test]
    fn done_and_undo_toggle_the_tick() {
        let mut store = Store::open_in_memory().unwrap();
        let item = add(&mut store, &config(), at(20, 9, 0), titled("Email")).unwrap();
        done(&mut store, &config(), at(20, 9, 30), &item.id[..8], None).unwrap();
        let view = today(&store, &config(), at(20, 9, 31)).unwrap();
        assert_eq!(view.items[0].state, State::Done);
        assert_eq!(view.open, 0);
        assert_eq!(
            undo(&mut store, &config(), at(20, 9, 40), &item.id, None).unwrap(),
            1
        );
        let view = today(&store, &config(), at(20, 9, 41)).unwrap();
        assert_eq!(view.items[0].state, State::Open);
    }

    #[test]
    fn a_running_timer_stops_at_the_end_of_its_block() {
        let mut store = Store::open_in_memory().unwrap();
        let quran = add(
            &mut store,
            &config(),
            at(20, 6, 30),
            NewItem {
                title: "Quran".into(),
                block: Some(BoundaryId::Fajr),
                repeat: Some(Repeat::Daily),
                measure: Some(Measure::Minutes),
                target: Some(500),
                ..NewItem::default()
            },
        )
        .unwrap();
        start_timer(&mut store, &config(), at(20, 11, 0), &quran.id).unwrap();
        // Well after Dhuhr, the Fajr block's timer has ended on its own.
        assert_eq!(
            close_expired_timers(&mut store, &config(), at(20, 15, 0)).unwrap(),
            1
        );
        let entry = &store.entries().unwrap()[0];
        let end = schedule::day_blocks(&config(), date(20)).unwrap()[1].end;
        assert_eq!(entry.ended_at, Some(end));
        let view = today(&store, &config(), at(20, 15, 0)).unwrap();
        let row = view.items.iter().find(|r| r.id == quran.id).unwrap();
        assert_eq!(row.minutes, (end - at(20, 11, 0)).num_minutes());
        assert!(row.timer_started_at.is_none());
    }

    #[test]
    fn starting_a_timer_stops_the_previous_one() {
        let mut store = Store::open_in_memory().unwrap();
        let minutes = |title: &str| NewItem {
            title: title.into(),
            measure: Some(Measure::Minutes),
            ..NewItem::default()
        };
        let a = add(&mut store, &config(), at(20, 13, 0), minutes("A")).unwrap();
        let b = add(&mut store, &config(), at(20, 13, 0), minutes("B")).unwrap();
        start_timer(&mut store, &config(), at(20, 13, 0), &a.id).unwrap();
        start_timer(&mut store, &config(), at(20, 13, 20), &b.id).unwrap();
        let running = store.running_timers().unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].item_id, b.id);
        let view = today(&store, &config(), at(20, 13, 30)).unwrap();
        let minutes_of = |id: &str| view.items.iter().find(|r| r.id == id).unwrap().minutes;
        assert_eq!((minutes_of(&a.id), minutes_of(&b.id)), (20, 10));
    }

    #[test]
    fn invalid_requests_change_nothing() {
        let mut store = Store::open_in_memory().unwrap();
        let tick = add(&mut store, &config(), at(20, 13, 0), titled("Tick")).unwrap();
        assert!(add(&mut store, &config(), at(20, 13, 0), titled("  ")).is_err());
        let sleep = NewItem {
            block: Some(BoundaryId::Sleep),
            ..titled("Nap")
        };
        assert!(add(&mut store, &config(), at(20, 13, 0), sleep).is_err());
        let target = NewItem {
            target: Some(5),
            ..titled("Target")
        };
        assert!(add(&mut store, &config(), at(20, 13, 0), target).is_err());
        assert!(log(&mut store, &config(), at(20, 13, 0), &tick.id, 0, None).is_err());
        assert!(log(&mut store, &config(), at(20, 13, 0), &tick.id, 1441, None).is_err());
        assert!(find_item(&store, "abc").is_err());
        assert_eq!(store.items().unwrap().len(), 1);
        assert!(store.entries().unwrap().is_empty());
    }

    #[test]
    fn ids_match_by_unique_prefix_only() {
        let mut store = Store::open_in_memory().unwrap();
        let mut item = add(&mut store, &config(), at(20, 13, 0), titled("One")).unwrap();
        item.id = "abcd1111".into();
        store.put_item(&item).unwrap();
        item.id = "abcd2222".into();
        store.put_item(&item).unwrap();
        assert!(find_item(&store, "abcd").unwrap_err().contains("ambiguous"));
        assert_eq!(find_item(&store, "abcd1").unwrap().id, "abcd1111");
        assert!(find_item(&store, "abc").is_err(), "too short");
    }

    #[test]
    fn moving_reorders_within_and_between_blocks() {
        let mut store = Store::open_in_memory().unwrap();
        let in_block = |title: &str, block| NewItem {
            block: Some(block),
            ..titled(title)
        };
        let a = add(
            &mut store,
            &config(),
            at(20, 13, 0),
            in_block("A", BoundaryId::Asr),
        )
        .unwrap();
        let b = add(
            &mut store,
            &config(),
            at(20, 13, 0),
            in_block("B", BoundaryId::Asr),
        )
        .unwrap();
        let c = add(
            &mut store,
            &config(),
            at(20, 13, 0),
            in_block("C", BoundaryId::Isha),
        )
        .unwrap();
        assert_eq!((a.position, b.position), (0, 1));
        move_item(&mut store, at(20, 13, 1), &b.id, BoundaryId::Asr, 0).unwrap();
        move_item(&mut store, at(20, 13, 2), &c.id, BoundaryId::Asr, 1).unwrap();
        let plan = day_plan(&store, &config(), at(20, 13, 3), None).unwrap();
        let asr: Vec<_> = plan.blocks[3]
            .items
            .iter()
            .map(|i| i.title.as_str())
            .collect();
        assert_eq!(asr, ["B", "C", "A"]);
        assert!(plan.blocks[5].items.is_empty());
    }

    #[test]
    fn prayers_can_be_marked_and_unmarked_idempotently() {
        let mut store = Store::open_in_memory().unwrap();
        mark(
            &mut store,
            &config(),
            at(20, 13, 0),
            BoundaryId::Dhuhr,
            None,
        )
        .unwrap();
        mark(
            &mut store,
            &config(),
            at(20, 13, 1),
            BoundaryId::Dhuhr,
            None,
        )
        .unwrap();
        mark(
            &mut store,
            &config(),
            at(20, 13, 2),
            BoundaryId::Fajr,
            Some(date(19)),
        )
        .unwrap();
        assert_eq!(store.marks().unwrap().len(), 2);
        let plan = day_plan(&store, &config(), at(20, 13, 3), None).unwrap();
        let marked: Vec<_> = plan
            .prayers
            .iter()
            .filter(|p| p.marked)
            .map(|p| p.id)
            .collect();
        assert_eq!(marked, [BoundaryId::Dhuhr]);
        unmark(
            &mut store,
            &config(),
            at(20, 13, 4),
            BoundaryId::Dhuhr,
            None,
        )
        .unwrap();
        assert!(
            today(&store, &config(), at(20, 13, 5))
                .unwrap()
                .prayer_marks
                .is_empty()
        );
        assert!(
            mark(
                &mut store,
                &config(),
                at(20, 13, 6),
                BoundaryId::Sleep,
                None
            )
            .is_err()
        );
    }

    #[test]
    fn day_plan_describes_the_timeline() {
        let mut store = Store::open_in_memory().unwrap();
        add(&mut store, &config(), at(20, 13, 0), titled("Email")).unwrap();
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

    #[test]
    fn ticks_can_be_recorded_on_another_day() {
        let mut store = Store::open_in_memory().unwrap();
        let walk = NewItem {
            repeat: Some(Repeat::Daily),
            date: Some(date(18)),
            ..titled("Walk")
        };
        let walk = add(&mut store, &config(), at(20, 13, 0), walk).unwrap();
        assert_eq!((walk.day, walk.block), (date(18), BoundaryId::Fajr));
        done(
            &mut store,
            &config(),
            at(20, 13, 0),
            &walk.id,
            Some(date(19)),
        )
        .unwrap();
        let days = history(&store, &config(), at(20, 13, 0), date(18), date(20)).unwrap();
        let states: Vec<_> = days.iter().map(|d| d.items[0].state).collect();
        assert_eq!(states, [State::Missed, State::Done, State::Open]);
    }

    #[test]
    fn stats_count_planned_and_done_days_and_prayers() {
        let mut store = Store::open_in_memory().unwrap();
        let read = NewItem {
            repeat: Some(Repeat::Weekly),
            weekdays: Some(Weekdays::WORKWEEK),
            measure: Some(Measure::Minutes),
            date: Some(date(16)),
            ..titled("Read")
        };
        let read = add(&mut store, &config(), at(20, 13, 0), read).unwrap();
        log(
            &mut store,
            &config(),
            at(20, 13, 0),
            &read.id,
            25,
            Some(date(17)),
        )
        .unwrap();
        mark(
            &mut store,
            &config(),
            at(20, 13, 0),
            BoundaryId::Asr,
            Some(date(17)),
        )
        .unwrap();
        // 2026-11-16 is a Monday: the range holds Mon–Sun.
        let report = stats(&store, &config(), at(20, 13, 0), date(16), date(22)).unwrap();
        assert_eq!(report.days, 7);
        let item = &report.items[0];
        assert_eq!((item.planned_days, item.minutes), (5, 25));
        assert_eq!(item.series.len(), 7);
        assert!(!item.series[5].planned, "Saturday is not planned");
        assert_eq!(report.prayers[2].marked_days, 1);
        assert_eq!((report.marked_total, report.prayer_slots), (1, 35));
        assert_eq!(report.daily.len(), 7);
        let tuesday = &report.daily[1];
        assert_eq!(
            (tuesday.planned, tuesday.minutes, tuesday.prayers),
            (1, 25, 1)
        );
        assert_eq!(report.daily[5].planned, 0);
        assert_eq!((report.totals.planned, report.totals.minutes), (5, 25));
        assert_eq!((report.totals.prayers, report.totals.active_days), (1, 1));
    }

    #[test]
    fn any_item_can_be_timed_and_adjusted_without_going_below_zero() {
        let mut store = Store::open_in_memory().unwrap();
        let email = add(&mut store, &config(), at(20, 13, 0), titled("Email")).unwrap();
        start_timer(&mut store, &config(), at(20, 13, 0), &email.id).unwrap();
        let view = today(&store, &config(), at(20, 13, 12)).unwrap();
        let timer = view.timer.unwrap();
        assert_eq!(
            (timer.item_id.as_str(), timer.minutes),
            (email.id.as_str(), 12)
        );
        stop_timer(&mut store, &config(), at(20, 13, 12)).unwrap();
        // Pause, then continue: the minutes add up.
        start_timer(&mut store, &config(), at(20, 13, 30), &email.id).unwrap();
        stop_timer(&mut store, &config(), at(20, 13, 38)).unwrap();
        log(&mut store, &config(), at(20, 13, 40), &email.id, 5, None).unwrap();
        log(&mut store, &config(), at(20, 13, 41), &email.id, -10, None).unwrap();
        let view = today(&store, &config(), at(20, 13, 42)).unwrap();
        assert!(view.timer.is_none());
        assert_eq!(view.items[0].minutes, 12 + 8 + 5 - 10);
        log(&mut store, &config(), at(20, 13, 43), &email.id, -100, None).unwrap();
        let view = today(&store, &config(), at(20, 13, 44)).unwrap();
        assert_eq!(view.items[0].minutes, 0, "clamped at zero");
        assert_eq!(
            view.items[0].state,
            State::Open,
            "time alone does not tick a check item"
        );

        let pushups = NewItem {
            measure: Some(Measure::Count),
            ..titled("Pushups")
        };
        let pushups = add(&mut store, &config(), at(20, 13, 0), pushups).unwrap();
        log(&mut store, &config(), at(20, 13, 50), &pushups.id, 3, None).unwrap();
        log(&mut store, &config(), at(20, 13, 51), &pushups.id, -1, None).unwrap();
        let view = today(&store, &config(), at(20, 13, 52)).unwrap();
        let row = view.items.iter().find(|r| r.id == pushups.id).unwrap();
        assert_eq!((row.count, row.minutes), (2, 0));
    }

    #[test]
    fn stats_list_routines_before_their_first_occurrence() {
        let mut store = Store::open_in_memory().unwrap();
        // 2026-11-22 is a Sunday; the routine starts on Monday.
        let gym = NewItem {
            repeat: Some(Repeat::Weekly),
            weekdays: Some(Weekdays::parse_list("mon,wed").unwrap()),
            ..titled("Gym")
        };
        add(&mut store, &config(), at(22, 13, 0), gym).unwrap();
        add(&mut store, &config(), at(22, 13, 0), titled("Once")).unwrap();
        let report = stats(&store, &config(), at(22, 13, 0), date(22), date(22)).unwrap();
        let titles: Vec<_> = report
            .items
            .iter()
            .map(|i| (i.title.as_str(), i.planned_days))
            .collect();
        assert!(titles.contains(&("Gym", 0)));
        assert!(titles.contains(&("Once", 1)));
        let earlier = stats(&store, &config(), at(22, 13, 0), date(20), date(21)).unwrap();
        assert!(earlier.items.iter().all(|i| i.title != "Once"));
    }

    #[test]
    fn removed_items_disappear_but_keep_a_tombstone() {
        let mut store = Store::open_in_memory().unwrap();
        let item = add(&mut store, &config(), at(20, 13, 0), titled("Gone")).unwrap();
        remove(&mut store, at(20, 13, 1), &item.id).unwrap();
        assert!(
            today(&store, &config(), at(20, 13, 2))
                .unwrap()
                .items
                .is_empty()
        );
        assert!(store.item(&item.id).unwrap().unwrap().deleted_at.is_some());
        assert!(find_item(&store, &item.id).is_err());
    }
}
