use super::items::find_item;
use super::{recorded_block, stamp};
use crate::config::Config;
use crate::error::Error;
use crate::item::{Entry, EntryKind, Item, Measure};
use crate::runtime::IdGenerator;
use crate::schedule::{self, BlockKey};
use crate::store::Store;
use chrono::{DateTime, NaiveDate, Utc};

pub(super) fn new_entry(
    item: &Item,
    block: BlockKey,
    kind: EntryKind,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
) -> Entry {
    Entry {
        id: ids.new_id(),
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
    ids: &dyn IdGenerator,
    id: &str,
    date: Option<NaiveDate>,
) -> Result<Entry, Error> {
    let now = stamp(now);
    let item = find_item(store, id)?;
    let schedule = schedule::resolve(config, now)?;
    let block = recorded_block(&schedule, &item, date);
    let entry = new_entry(&item, block, EntryKind::Done, now, ids);
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
) -> Result<usize, Error> {
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
    ids: &dyn IdGenerator,
    id: &str,
    value: i64,
    date: Option<NaiveDate>,
) -> Result<Entry, Error> {
    let item = find_item(store, id)?;
    // Count items adjust their count; every other item records time spent.
    let kind = match item.measure {
        Measure::Count => EntryKind::Count,
        Measure::Check | Measure::Minutes => EntryKind::Minutes,
    };
    record(store, config, now, ids, &item, (kind, value), date)
}

/// Adds (or, when negative, subtracts) minutes of time spent on any item,
/// including count items, whose `log` adjusts the count instead.
pub fn adjust_time(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    id: &str,
    value: i64,
    date: Option<NaiveDate>,
) -> Result<Entry, Error> {
    let item = find_item(store, id)?;
    record(
        store,
        config,
        now,
        ids,
        &item,
        (EntryKind::Minutes, value),
        date,
    )
}

fn record(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    item: &Item,
    (kind, value): (EntryKind, i64),
    date: Option<NaiveDate>,
) -> Result<Entry, Error> {
    let now = stamp(now);
    // A negative value subtracts; daily totals are clamped at zero.
    if value == 0 || value.abs() > 24 * 60 {
        return Err("value must be between -1440 and 1440, and not 0".into());
    }
    let schedule = schedule::resolve(config, now)?;
    let mut entry = new_entry(item, recorded_block(&schedule, item, date), kind, now, ids);
    entry.value = Some(value);
    store.put_entry(&entry)?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::day::BoundaryId;
    use crate::item::{Repeat, State};
    use crate::planner::NewItem;
    use crate::planner::test_support::{at, config, date, ids, titled};
    use crate::planner::{add, history, today};

    #[test]
    fn done_and_undo_toggle_the_tick() {
        let mut store = Store::open_in_memory().unwrap();
        let item = add(&mut store, &config(), at(20, 9, 0), ids(), titled("Email")).unwrap();
        done(
            &mut store,
            &config(),
            at(20, 9, 30),
            ids(),
            &item.id[..8],
            None,
        )
        .unwrap();
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
    fn ticks_can_be_recorded_on_another_day() {
        let mut store = Store::open_in_memory().unwrap();
        let walk = NewItem {
            repeat: Some(Repeat::Daily),
            date: Some(date(18)),
            ..titled("Walk")
        };
        let walk = add(&mut store, &config(), at(20, 13, 0), ids(), walk).unwrap();
        assert_eq!((walk.day, walk.block), (date(18), BoundaryId::Fajr));
        done(
            &mut store,
            &config(),
            at(20, 13, 0),
            ids(),
            &walk.id,
            Some(date(19)),
        )
        .unwrap();
        let days = history(&store, &config(), at(20, 13, 0), date(18), date(20)).unwrap();
        let states: Vec<_> = days.iter().map(|d| d.items[0].state).collect();
        assert_eq!(states, [State::Missed, State::Done, State::Open]);
    }
}
