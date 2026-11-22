use super::entries::new_entry;
use super::items::find_item;
use super::{BlockEnds, stamp};
use crate::config::Config;
use crate::error::Error;
use crate::item::{Entry, EntryKind};
use crate::runtime::IdGenerator;
use crate::schedule;
use crate::store::Store;
use chrono::{DateTime, Utc};

/// Persists the automatic stop of timers whose block has ended.
pub fn close_expired_timers(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
) -> Result<usize, Error> {
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

/// Starts a timer for a minutes item, stopping any other running timer.
pub fn start_timer(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    id: &str,
) -> Result<Entry, Error> {
    let now = stamp(now);
    let item = find_item(store, id)?;
    let schedule = schedule::resolve(config, now)?;
    close_expired_timers(store, config, now)?;
    let mut timer = new_entry(&item, schedule.current_block(), EntryKind::Timer, now, ids);
    timer.started_at = Some(now);
    store.transaction(|store| {
        stop_running(store, now)?;
        store.put_entry(&timer)?;
        Ok(())
    })?;
    Ok(timer)
}

fn stop_running(store: &Store, now: DateTime<Utc>) -> Result<Vec<Entry>, Error> {
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
) -> Result<Vec<Entry>, Error> {
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
) -> Result<Vec<Entry>, Error> {
    let now = stamp(now);
    close_expired_timers(store, config, now)?;
    store.transaction(|store| stop_running(store, now))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::day::BoundaryId;
    use crate::item::{Measure, Repeat, State};
    use crate::planner::NewItem;
    use crate::planner::test_support::{at, config, date, ids, titled};
    use crate::planner::{add, log, today};

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
