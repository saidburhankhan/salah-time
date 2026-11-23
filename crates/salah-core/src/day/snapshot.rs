//! Day snapshots. Days from today on are computed live from the current
//! configuration and items and are never stored ahead. Once a day is past its
//! blocks and plan are written once, from the items that existed by the day's
//! end, and then never change.

use crate::config::Config;
use crate::{Store, WriteTx};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use salah_domain::{
    day::BoundaryId,
    error::Error,
    tracking::{DayBlock, DayBlockSource, DayClock, DayPlanLine, Item},
};
use salah_prayer::schedule;

/// A day's blocks and plan lines.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub clock: DayClock,
    pub blocks: Vec<DayBlock>,
    pub plan: Vec<DayPlanLine>,
}

/// The stored snapshot of a written past day, else the computed one.
pub fn snapshot(
    store: &Store,
    config: &Config,
    day: NaiveDate,
    today: NaiveDate,
) -> Result<Snapshot, Error> {
    if day < today && store.day_written(day)? {
        return Ok(Snapshot {
            blocks: store.day_blocks(day)?,
            clock: clock_for_day(store, config, day, today)?,
            plan: store.day_plan(day)?,
        });
    }
    let (blocks, clock) = blocks_for_day(store, config, day, today)?;
    let items = if day < today {
        store.items_on(day)?
    } else {
        store.items()?
    };
    let plan = planned_lines(items, day, clock.sleep_at);
    Ok(Snapshot {
        blocks,
        clock,
        plan,
    })
}

/// Resolves the same Sleep instant for day rows, running timers, and stops
/// without loading the day's items or plan.
pub(crate) fn clock_for_day(
    store: &Store,
    config: &Config,
    day: NaiveDate,
    today: NaiveDate,
) -> Result<DayClock, Error> {
    if day < today
        && let Some(clock) = store.day_clock(day)?
    {
        return Ok(clock);
    }
    let (_, mut clock) = blocks_for_day(store, config, day, today)?;
    if day < today
        && let Some(night) = store
            .day_blocks(day)?
            .iter()
            .find(|b| b.block == BoundaryId::Night)
    {
        // Legacy snapshots only contain UTC block windows. Both nights
        // used the same fixed offset and sleep time.
        clock.sleep_at = night.starts_at + Duration::days(1);
    }
    Ok(clock)
}

/// Writes the snapshot of a past day that was never written; days from
/// `today` on are left alone.
pub fn freeze(
    tx: &WriteTx<'_>,
    config: &Config,
    day: NaiveDate,
    today: NaiveDate,
) -> Result<(), Error> {
    if day >= today {
        return Ok(());
    }
    if tx.day_written(day)? {
        if tx.day_clock(day)?.is_none() {
            tx.put_day_clock(day, &snapshot(tx, config, day, today)?.clock)?;
        }
        return Ok(());
    }
    let Snapshot {
        blocks,
        clock,
        plan,
    } = snapshot(tx, config, day, today)?;
    tx.write_day(day, &clock, &blocks, &plan)
}

/// Freezes unwritten days in the past year before an item changes. Reads may
/// have materialized isolated days anywhere in that range.
pub fn freeze_past(tx: &WriteTx<'_>, config: &Config, today: NaiveDate) -> Result<(), Error> {
    let Some(first) = tx.items()?.iter().map(|item| item.day).min() else {
        return Ok(());
    };
    let from = first.max(today - Duration::days(366));
    let written = tx.written_days(from, today)?;
    from.iter_days()
        .take_while(|day| *day < today)
        .filter(|day| !written.contains(day))
        .try_for_each(|day| freeze(tx, config, day, today))
}

/// The length of one block in minutes.
pub fn block_minutes(block: &DayBlock) -> i64 {
    (block.ends_at - block.starts_at).num_minutes().max(0)
}

/// A day's waking capacity: every block but the night (sleep to Fajr).
pub fn day_capacity<'a>(blocks: impl IntoIterator<Item = &'a DayBlock>) -> i64 {
    blocks
        .into_iter()
        .filter(|block| block.block != BoundaryId::Night)
        .map(block_minutes)
        .sum()
}

fn blocks_for_day(
    store: &Store,
    config: &Config,
    day: NaiveDate,
    today: NaiveDate,
) -> Result<(Vec<DayBlock>, DayClock), Error> {
    let historical: Option<Config> = if day < today {
        store
            .historical_config(day)?
            .map(|body| serde_json::from_str(&body))
            .transpose()
            .map_err(|e| Error::Storage(e.to_string()))?
    } else {
        None
    };
    let schedule = schedule::day_schedule(historical.as_ref().unwrap_or(config), day)?;
    let clock = DayClock {
        offset_seconds: schedule.offset_seconds,
        sunrise_minutes: schedule
            .prayers
            .iter()
            .find(|p| p.id == "sunrise")
            .map(|p| p.minutes),
        sleep_at: schedule.sleep_at,
    };
    let source = if schedule.source.id == "official" {
        DayBlockSource::Official
    } else {
        DayBlockSource::Calculated
    };
    let blocks = schedule
        .blocks
        .into_iter()
        .map(|window| DayBlock {
            day,
            block: window.key.start,
            starts_at: window.start,
            ends_at: window.end,
            source,
        })
        .collect();
    Ok((blocks, clock))
}

/// The lines of the items that existed at the day's end and recur on it.
fn planned_lines(items: Vec<Item>, day: NaiveDate, end: DateTime<Utc>) -> Vec<DayPlanLine> {
    let mut lines: Vec<DayPlanLine> = items
        .into_iter()
        .filter(|item| {
            item.created_at <= end
                && item.deleted_at.is_none_or(|at| at > end)
                && item.recurs_on(day)
        })
        .map(|item| DayPlanLine {
            day,
            block: item.block,
            item_id: item.id,
            project_id: item.project_id,
            planned_minutes: item.planned_minutes,
        })
        .collect();
    lines.sort_by(|a, b| (a.block, &a.item_id).cmp(&(b.block, &b.item_id)));
    lines
}
