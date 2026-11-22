//! Planner use cases: the only place that combines the schedule, the item
//! rules, and the store. The CLI, the widget, and the desktop app all go
//! through these functions, so behaviour is identical everywhere.

use crate::config::Config;
use crate::day::BoundaryId;
use crate::item::{
    self, DayItem, DayView, Entry, EntryKind, Item, Measure, Repeat, State, validate_block,
    validate_title,
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

/// The block that activity happening now is recorded in. While sleeping it
/// is the ended day's Isha block: a late tick still belongs to that day.
fn activity_block(schedule: &Schedule) -> BlockKey {
    if schedule.timeline.sleeping() {
        BlockKey {
            day: schedule.day(),
            start: BoundaryId::Isha,
        }
    } else {
        schedule.current_block()
    }
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
    pub measure: Option<Measure>,
    pub target: Option<i64>,
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
    let item = Item {
        id: new_id(),
        title: validate_title(&request.title)?,
        block: validate_block(request.block.unwrap_or(planning.start))?,
        repeat: request.repeat.unwrap_or(Repeat::None),
        measure,
        target: request.target,
        day: planning.day,
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
) -> Result<Entry, String> {
    let now = stamp(now);
    let item = find_item(store, id)?;
    let schedule = schedule::resolve(config, now)?;
    let entry = new_entry(&item, entry_block(&schedule, &item), EntryKind::Done, now);
    store.put_entry(&entry)?;
    Ok(entry)
}

/// Removes the tick from an item's day. Logged minutes and counts stay.
pub fn undo(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
) -> Result<usize, String> {
    let now = stamp(now);
    let item = find_item(store, id)?;
    let schedule = schedule::resolve(config, now)?;
    let day = entry_block(&schedule, &item).day;
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
) -> Result<Entry, String> {
    let now = stamp(now);
    let item = find_item(store, id)?;
    let kind = match item.measure {
        Measure::Minutes => EntryKind::Minutes,
        Measure::Count => EntryKind::Count,
        Measure::Check => return Err("this item is ticked, not measured; use `done`".into()),
    };
    if !(1..=24 * 60).contains(&value) {
        return Err("value must be between 1 and 1440".into());
    }
    let schedule = schedule::resolve(config, now)?;
    let mut entry = new_entry(&item, entry_block(&schedule, &item), kind, now);
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
    if item.measure != Measure::Minutes {
        return Err("timers are for minutes items".into());
    }
    let schedule = schedule::resolve(config, now)?;
    if schedule.timeline.sleeping() {
        return Err("the day has ended; timers start again at Fajr".into());
    }
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
    /// The block in progress; `None` while sleeping.
    pub block: Option<BoundaryId>,
    /// Where `add` places a new item right now.
    pub planning_block: BlockKey,
    /// Items for the block in progress, carried-forward ones included.
    pub current: Vec<DayItem>,
    /// Open items in `current`.
    pub open: usize,
    /// Every item planned today.
    pub items: Vec<DayItem>,
    /// Items planned for the coming day while sleeping.
    pub upcoming: Vec<DayItem>,
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
    let day = schedule.day();
    let sleeping = schedule.timeline.sleeping();
    let planning = schedule.planning_block();
    let items = store.items()?;
    let entries = store.entries_between(day, planning.day)?;
    let mut ends = BlockEnds::new(config);
    let block = (!sleeping).then(|| schedule.current_block().start);
    let rows = report_day(
        &items,
        &entries,
        &mut ends,
        DayView {
            day,
            current: block,
            ended: false,
            now,
        },
    );
    let upcoming = if sleeping {
        report_day(
            &items,
            &entries,
            &mut ends,
            DayView {
                day: planning.day,
                current: None,
                ended: false,
                now,
            },
        )
    } else {
        Vec::new()
    };
    let current = block
        .map(|b| item::current_block_items(&rows, b))
        .unwrap_or_default();
    let open = current.iter().filter(|r| r.state == State::Open).count();
    Ok(Today {
        day,
        block,
        planning_block: planning,
        current,
        open,
        items: rows,
        upcoming,
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
    let current = (!schedule.timeline.sleeping()).then(|| schedule.current_block().start);
    let items = store.items()?;
    let entries = store.entries_between(first, last)?;
    let mut ends = BlockEnds::new(config);
    let mut days = Vec::new();
    let mut day = first;
    while day <= last {
        let view = DayView {
            day,
            current: (day == today).then_some(current).flatten(),
            // Today ends at sleep; once sleeping, its outcome is final.
            ended: day < today || (day == today && schedule.timeline.sleeping()),
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
    fn adding_while_sleeping_plans_the_coming_fajr_block() {
        let mut store = Store::open_in_memory().unwrap();
        let item = add(&mut store, &config(), at(21, 2, 0), titled("Pack bag")).unwrap();
        assert_eq!((item.day, item.block), (date(21), BoundaryId::Fajr));
        let view = today(&store, &config(), at(21, 2, 1)).unwrap();
        assert!(view.block.is_none());
        assert_eq!(view.upcoming.len(), 1);
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
        done(&mut store, &config(), at(20, 9, 30), &item.id[..8]).unwrap();
        let view = today(&store, &config(), at(20, 9, 31)).unwrap();
        assert_eq!(view.items[0].state, State::Done);
        assert_eq!(view.open, 0);
        assert_eq!(
            undo(&mut store, &config(), at(20, 9, 40), &item.id).unwrap(),
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
        let end = schedule::day_blocks(&config(), date(20)).unwrap()[0].end;
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
        assert!(log(&mut store, &config(), at(20, 13, 0), &tick.id, 5).is_err());
        assert!(start_timer(&mut store, &config(), at(20, 13, 0), &tick.id).is_err());
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
