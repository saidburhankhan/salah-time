use super::entries::new_entry;
use super::items::find_item;
use super::{BlockEnds, Mutation, StoredResult, VersionedEntry, stamp, write_mutation};
use crate::config::Config;
use chrono::{DateTime, Utc};
use salah_domain::error::Error;
use salah_domain::item::{Entry, EntryKind};
use salah_domain::runtime::IdGenerator;
use salah_prayer::schedule;
use salah_store::{Store, WriteTx};
use serde::{Deserialize, Serialize};

/// Persists the automatic stop of timers whose block has ended. Callers reach
/// this only from inside a write transaction; reads derive expiry instead.
pub fn close_expired_timers(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
) -> Result<usize, Error> {
    let now = stamp(now);
    let mut ends = BlockEnds::new(config);
    let expired: Vec<(Entry, DateTime<Utc>)> = tx
        .running_timers()?
        .into_iter()
        .filter_map(|timer| {
            let end = ends.end(timer.block)?;
            (end <= now).then_some((timer, end))
        })
        .collect();
    for (mut timer, end) in expired.iter().cloned() {
        timer.ended_at = Some(end.max(timer.started_at.unwrap_or(end)));
        timer.updated_at = now;
        tx.put_entry(&timer)?;
    }
    Ok(expired.len())
}

/// A started timer plus every timer it displaced, so callers can report all
/// affected rows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StartedTimer {
    pub timer: Entry,
    pub stopped: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
struct StoredStartedTimer {
    timer: VersionedEntry,
    stopped: Vec<VersionedEntry>,
}

impl StoredResult for StartedTimer {
    fn to_stored_json(&self) -> Result<String, Error> {
        let stored = StoredStartedTimer {
            timer: VersionedEntry {
                row: self.timer.clone(),
                version: self.timer.version,
            },
            stopped: self
                .stopped
                .iter()
                .map(|row| VersionedEntry {
                    row: row.clone(),
                    version: row.version,
                })
                .collect(),
        };
        serde_json::to_string(&stored).map_err(|error| Error::Internal(format!("store: {error}")))
    }

    fn from_stored_json(text: &str) -> Result<Self, Error> {
        let Ok(stored) = serde_json::from_str::<StoredStartedTimer>(text) else {
            // Rows written before the stopped-timer list was added contain a
            // single entry; replay them as a start that displaced nothing.
            return Ok(Self {
                timer: Entry::from_stored_json(text)?,
                stopped: Vec::new(),
            });
        };
        let mut timer = stored.timer.row;
        timer.version = stored.timer.version;
        Ok(Self {
            timer,
            stopped: stored
                .stopped
                .into_iter()
                .map(|stored| {
                    let mut row = stored.row;
                    row.version = stored.version;
                    row
                })
                .collect(),
        })
    }
}

/// Starts a timer for a minutes item, stopping any other running timer.
pub fn start_timer(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    id: &str,
) -> Result<Entry, Error> {
    start_timer_with(store, config, now, ids, id, Mutation::default())
}

pub fn start_timer_with(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    id: &str,
    mutation: Mutation<'_>,
) -> Result<Entry, Error> {
    start_timer_with_stopped(store, config, now, ids, id, mutation).map(|started| started.timer)
}

/// Starts a timer and returns the stopped timers as well.
pub fn start_timer_with_stopped(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    id: &str,
    mutation: Mutation<'_>,
) -> Result<StartedTimer, Error> {
    let now = stamp(now);
    let schedule = schedule::resolve(config, now)?;
    write_mutation(store, config, now, mutation, |tx| {
        let item = find_item(tx, id)?;
        let mut timer = new_entry(&item, schedule.current_block(), EntryKind::Timer, now, ids);
        timer.started_at = Some(now);
        let stopped = stop_running(tx, now)?;
        timer.version = tx.put_entry(&timer)?;
        Ok(StartedTimer { timer, stopped })
    })
}

fn stop_running(tx: &WriteTx<'_>, now: DateTime<Utc>) -> Result<Vec<Entry>, Error> {
    let mut stopped = Vec::new();
    for mut timer in tx.running_timers()? {
        timer.ended_at = Some(now);
        timer.updated_at = now;
        timer.version = tx.put_entry(&timer)?;
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
) -> Result<Vec<Entry>, Error> {
    stop_item_timer_with(store, config, now, id, Mutation::default())
}

pub fn stop_item_timer_with(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    mutation: Mutation<'_>,
) -> Result<Vec<Entry>, Error> {
    let now = stamp(now);
    write_mutation(store, config, now, mutation, |tx| {
        let item = find_item(tx, id)?;
        let mut stopped = Vec::new();
        for mut timer in tx.running_timers()? {
            if timer.item_id == item.id {
                timer.ended_at = Some(now);
                timer.updated_at = now;
                timer.version = tx.put_entry(&timer)?;
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
) -> Result<Vec<Entry>, Error> {
    stop_timer_with(store, config, now, Mutation::default())
}

pub fn stop_timer_with(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    mutation: Mutation<'_>,
) -> Result<Vec<Entry>, Error> {
    let now = stamp(now);
    write_mutation(store, config, now, mutation, |tx| stop_running(tx, now))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::NewItem;
    use crate::planner::test_support::{at, config, date, ids, titled};
    use crate::planner::{Mutation, add, day_plan, history, log, stats, stop_timer_with, today};
    use salah_domain::day::BoundaryId;
    use salah_domain::item::{Measure, Repeat, State};

    #[test]
    fn a_running_timer_stops_at_the_end_of_its_block() {
        let mut store = Store::open_in_memory().unwrap();
        let quran = add(
            &mut store,
            &config(),
            at(20, 6, 30),
            ids(),
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
        start_timer(&mut store, &config(), at(20, 11, 0), ids(), &quran.id).unwrap();
        // Well after Dhuhr, the Fajr block's timer has ended on its own.
        assert_eq!(
            store
                .write(|tx| close_expired_timers(tx, &config(), at(20, 15, 0)))
                .unwrap(),
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
    fn read_paths_derive_expiry_without_writing() {
        let mut store = Store::open_in_memory().unwrap();
        let quran = add(
            &mut store,
            &config(),
            at(20, 6, 30),
            ids(),
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
        start_timer(&mut store, &config(), at(20, 11, 0), ids(), &quran.id).unwrap();
        let before = store.entries().unwrap();

        let now = at(20, 15, 0);
        today(&store, &config(), now).unwrap();
        day_plan(&store, &config(), now, None).unwrap();
        history(&store, &config(), now, date(20), date(20)).unwrap();
        stats(&store, &config(), now, date(20), date(20)).unwrap();
        crate::export::export(&store, now).unwrap();

        assert_eq!(store.entries().unwrap(), before, "reads must not write");
        assert_eq!(
            store.running_timers().unwrap().len(),
            1,
            "the timer stays open until a write transaction persists the close"
        );
    }

    #[test]
    fn starting_a_timer_stops_the_previous_one() {
        let mut store = Store::open_in_memory().unwrap();
        let minutes = |title: &str| NewItem {
            title: title.into(),
            measure: Some(Measure::Minutes),
            ..NewItem::default()
        };
        let a = add(&mut store, &config(), at(20, 13, 0), ids(), minutes("A")).unwrap();
        let b = add(&mut store, &config(), at(20, 13, 0), ids(), minutes("B")).unwrap();
        start_timer(&mut store, &config(), at(20, 13, 0), ids(), &a.id).unwrap();
        start_timer(&mut store, &config(), at(20, 13, 20), ids(), &b.id).unwrap();
        let running = store.running_timers().unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].item_id, b.id);
        let view = today(&store, &config(), at(20, 13, 30)).unwrap();
        let minutes_of = |id: &str| view.items.iter().find(|r| r.id == id).unwrap().minutes;
        assert_eq!((minutes_of(&a.id), minutes_of(&b.id)), (20, 10));
    }

    #[test]
    fn replayed_stop_returns_the_stored_entries_without_writing() {
        let mut store = Store::open_in_memory().unwrap();
        let email = add(
            &mut store,
            &config(),
            at(20, 13, 0),
            ids(),
            NewItem {
                measure: Some(Measure::Minutes),
                ..titled("Email")
            },
        )
        .unwrap();
        start_timer(&mut store, &config(), at(20, 13, 0), ids(), &email.id).unwrap();
        let first = stop_timer_with(
            &mut store,
            &config(),
            at(20, 13, 12),
            Mutation::new(Some("op-stop"), None),
        )
        .unwrap();
        let revision = store.revision().unwrap();
        let replay = stop_timer_with(
            &mut store,
            &config(),
            at(20, 13, 30),
            Mutation::new(Some("op-stop"), None),
        )
        .unwrap();
        assert_eq!(replay, first);
        assert_eq!(store.revision().unwrap(), revision);
        assert_eq!(store.entries().unwrap().len(), 1);
    }

    #[test]
    fn any_item_can_be_timed_and_adjusted_without_going_below_zero() {
        let mut store = Store::open_in_memory().unwrap();
        let email = add(&mut store, &config(), at(20, 13, 0), ids(), titled("Email")).unwrap();
        start_timer(&mut store, &config(), at(20, 13, 0), ids(), &email.id).unwrap();
        let view = today(&store, &config(), at(20, 13, 12)).unwrap();
        let timer = view.timer.unwrap();
        assert_eq!(
            (timer.item_id.as_str(), timer.minutes),
            (email.id.as_str(), 12)
        );
        stop_timer(&mut store, &config(), at(20, 13, 12)).unwrap();
        // Pause, then continue: the minutes add up.
        start_timer(&mut store, &config(), at(20, 13, 30), ids(), &email.id).unwrap();
        stop_timer(&mut store, &config(), at(20, 13, 38)).unwrap();
        log(
            &mut store,
            &config(),
            at(20, 13, 40),
            ids(),
            &email.id,
            5,
            None,
        )
        .unwrap();
        log(
            &mut store,
            &config(),
            at(20, 13, 41),
            ids(),
            &email.id,
            -10,
            None,
        )
        .unwrap();
        let view = today(&store, &config(), at(20, 13, 42)).unwrap();
        assert!(view.timer.is_none());
        assert_eq!(view.items[0].minutes, 12 + 8 + 5 - 10);
        log(
            &mut store,
            &config(),
            at(20, 13, 43),
            ids(),
            &email.id,
            -100,
            None,
        )
        .unwrap();
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
        let pushups = add(&mut store, &config(), at(20, 13, 0), ids(), pushups).unwrap();
        log(
            &mut store,
            &config(),
            at(20, 13, 50),
            ids(),
            &pushups.id,
            3,
            None,
        )
        .unwrap();
        log(
            &mut store,
            &config(),
            at(20, 13, 51),
            ids(),
            &pushups.id,
            -1,
            None,
        )
        .unwrap();
        let view = today(&store, &config(), at(20, 13, 52)).unwrap();
        let row = view.items.iter().find(|r| r.id == pushups.id).unwrap();
        assert_eq!((row.count, row.minutes), (2, 0));
    }
}
