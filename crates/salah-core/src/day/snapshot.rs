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
    tracking::{DayBlock, DayBlockSource, DayPlanLine, Item},
};
use salah_prayer::schedule;

/// A day's blocks and plan lines.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
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
            plan: store.day_plan(day)?,
        });
    }
    let blocks = scheduled_blocks(config, day)?;
    let plan = planned_lines(store, day, day_end(&blocks))?;
    Ok(Snapshot { blocks, plan })
}

/// Writes the snapshot of a past day that was never written; days from
/// `today` on are left alone.
pub fn freeze(
    tx: &WriteTx<'_>,
    config: &Config,
    day: NaiveDate,
    today: NaiveDate,
) -> Result<(), Error> {
    if day >= today || tx.day_written(day)? {
        return Ok(());
    }
    let Snapshot { blocks, plan } = snapshot(tx, config, day, today)?;
    tx.write_day(day, &blocks, &plan)
}

/// Freezes every past day not yet written, from the day after the last
/// written one (or the first item's day) up to yesterday, at most a year
/// back. Item writes call this first, so the plan of a day that has ended is
/// kept as it stood rather than rebuilt from later edits.
pub fn freeze_past(tx: &WriteTx<'_>, config: &Config, today: NaiveDate) -> Result<(), Error> {
    let first_item = tx.items()?.iter().map(|item| item.day).min();
    let Some(from) = tx
        .last_written_day()?
        .map(|day| day + Duration::days(1))
        .or(first_item)
    else {
        return Ok(());
    };
    let from = from.max(today - Duration::days(366));
    from.iter_days()
        .take_while(|day| *day < today)
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

/// The day ends at its sleep boundary, the end of the last block.
fn day_end(blocks: &[DayBlock]) -> Option<DateTime<Utc>> {
    blocks.iter().map(|block| block.ends_at).max()
}

fn scheduled_blocks(config: &Config, day: NaiveDate) -> Result<Vec<DayBlock>, Error> {
    let (_, source) = schedule::prayers_for(config, day)?;
    let source = if source.id == "official" {
        DayBlockSource::Official
    } else {
        DayBlockSource::Calculated
    };
    Ok(schedule::day_blocks(config, day)?
        .into_iter()
        .map(|window| DayBlock {
            day,
            block: window.key.start,
            starts_at: window.start,
            ends_at: window.end,
            source,
        })
        .collect())
}

/// The lines of the items that existed at the day's end and recur on it.
fn planned_lines(
    store: &Store,
    day: NaiveDate,
    end: Option<DateTime<Utc>>,
) -> Result<Vec<DayPlanLine>, Error> {
    let existed = |item: &Item| match end {
        Some(end) => item.created_at <= end && item.deleted_at.is_none_or(|at| at > end),
        None => item.is_live(),
    };
    let mut lines: Vec<DayPlanLine> = store
        .items()?
        .into_iter()
        .filter(|item| existed(item) && item.recurs_on(day))
        .map(|item| DayPlanLine {
            day,
            block: item.block,
            item_id: item.id,
            project_id: item.project_id,
            planned_minutes: item.planned_minutes,
        })
        .collect();
    lines.sort_by(|a, b| (a.block, &a.item_id).cmp(&(b.block, &b.item_id)));
    Ok(lines)
}
