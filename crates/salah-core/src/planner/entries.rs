use super::items::find_item;
use super::{Mutation, recorded_block, stamp, write_mutation};
use crate::config::Config;
use chrono::{DateTime, NaiveDate, Utc};
use salah_domain::day::BlockKey;
use salah_domain::error::Error;
use salah_domain::item::{Entry, EntryKind, Item, Measure};
use salah_domain::runtime::IdGenerator;
use salah_prayer::schedule;
use salah_store::Store;

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
        version: 1,
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
    done_with(store, config, now, ids, id, date, Mutation::default())
}

pub fn done_with(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    id: &str,
    date: Option<NaiveDate>,
    mutation: Mutation<'_>,
) -> Result<Entry, Error> {
    let now = stamp(now);
    let schedule = schedule::resolve(config, now)?;
    write_mutation(store, config, now, mutation, |tx| {
        let item = find_item(tx, id)?;
        let block = recorded_block(&schedule, &item, date);
        let mut entry = new_entry(&item, block, EntryKind::Done, now, ids);
        entry.version = tx.put_entry(&entry)?;
        Ok(entry)
    })
}

/// Removes the tick from an item's day. Logged minutes and counts stay.
pub fn undo(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    date: Option<NaiveDate>,
) -> Result<usize, Error> {
    undo_with(store, config, now, id, date, Mutation::default())
}

pub fn undo_with(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    date: Option<NaiveDate>,
    mutation: Mutation<'_>,
) -> Result<usize, Error> {
    let now = stamp(now);
    let schedule = schedule::resolve(config, now)?;
    write_mutation(store, config, now, mutation, |tx| {
        let item = find_item(tx, id)?;
        let day = recorded_block(&schedule, &item, date).day;
        let ticks: Vec<Entry> = tx
            .entries_between(day, day)?
            .into_iter()
            .filter(|e| e.is_live() && e.item_id == item.id && e.kind == EntryKind::Done)
            .collect();
        for mut tick in ticks.iter().cloned() {
            tick.deleted_at = Some(now);
            tick.updated_at = now;
            tx.put_entry(&tick)?;
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
    log_with(
        store,
        config,
        now,
        ids,
        id,
        value,
        date,
        Mutation::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn log_with(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    id: &str,
    value: i64,
    date: Option<NaiveDate>,
    mutation: Mutation<'_>,
) -> Result<Entry, Error> {
    // Count items adjust their count; every other item records time spent.
    record(store, config, now, ids, id, (None, value), date, mutation)
}

/// Records a count against any item, regardless of its own measure.
pub fn log_count(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    id: &str,
    value: i64,
    date: Option<NaiveDate>,
) -> Result<Entry, Error> {
    log_count_with(
        store,
        config,
        now,
        ids,
        id,
        value,
        date,
        Mutation::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn log_count_with(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    id: &str,
    value: i64,
    date: Option<NaiveDate>,
    mutation: Mutation<'_>,
) -> Result<Entry, Error> {
    record(
        store,
        config,
        now,
        ids,
        id,
        (Some(EntryKind::Count), value),
        date,
        mutation,
    )
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
    adjust_time_with(
        store,
        config,
        now,
        ids,
        id,
        value,
        date,
        Mutation::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn adjust_time_with(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    id: &str,
    value: i64,
    date: Option<NaiveDate>,
    mutation: Mutation<'_>,
) -> Result<Entry, Error> {
    record(
        store,
        config,
        now,
        ids,
        id,
        (Some(EntryKind::Minutes), value),
        date,
        mutation,
    )
}

#[allow(clippy::too_many_arguments)]
fn record(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    id: &str,
    (kind, value): (Option<EntryKind>, i64),
    date: Option<NaiveDate>,
    mutation: Mutation<'_>,
) -> Result<Entry, Error> {
    let now = stamp(now);
    // A negative value subtracts; daily totals are clamped at zero.
    if value == 0 || value.abs() > 24 * 60 {
        return Err("value must be between -1440 and 1440, and not 0".into());
    }
    let schedule = schedule::resolve(config, now)?;
    write_mutation(store, config, now, mutation, |tx| {
        let item = find_item(tx, id)?;
        let kind = kind.unwrap_or(match item.measure {
            Measure::Count => EntryKind::Count,
            Measure::Check | Measure::Minutes => EntryKind::Minutes,
        });
        let mut entry = new_entry(
            &item,
            recorded_block(&schedule, &item, date),
            kind,
            now,
            ids,
        );
        entry.value = Some(value);
        entry.version = tx.put_entry(&entry)?;
        Ok(entry)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::NewItem;
    use crate::planner::test_support::{at, config, date, ids, titled};
    use crate::planner::{Mutation, add, done_with, history, today, undo_with};
    use salah_domain::day::BoundaryId;
    use salah_domain::item::{Repeat, State};

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
    fn replayed_entry_ops_return_the_same_row_without_writing() {
        let mut store = Store::open_in_memory().unwrap();
        let item = add(&mut store, &config(), at(20, 9, 0), ids(), titled("Email")).unwrap();
        let first = done_with(
            &mut store,
            &config(),
            at(20, 9, 30),
            ids(),
            &item.id,
            None,
            Mutation::new(Some("op-done"), None),
        )
        .unwrap();
        let revision = store.revision().unwrap();
        let replay = done_with(
            &mut store,
            &config(),
            at(20, 9, 31),
            ids(),
            &item.id,
            None,
            Mutation::new(Some("op-done"), None),
        )
        .unwrap();
        assert_eq!(replay, first);
        assert_eq!(store.revision().unwrap(), revision);
        assert_eq!(store.entries().unwrap().len(), 1);

        let undone = undo_with(
            &mut store,
            &config(),
            at(20, 9, 40),
            &item.id,
            None,
            Mutation::new(Some("op-undo"), None),
        )
        .unwrap();
        assert_eq!(undone, 1);
        let revision = store.revision().unwrap();
        let replay = undo_with(
            &mut store,
            &config(),
            at(20, 9, 41),
            &item.id,
            None,
            Mutation::new(Some("op-undo"), None),
        )
        .unwrap();
        assert_eq!(replay, 1);
        assert_eq!(store.revision().unwrap(), revision);
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
