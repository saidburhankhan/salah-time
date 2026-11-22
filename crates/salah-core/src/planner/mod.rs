//! Planner use cases: the only place that combines the schedule, the item
//! rules, and the store. The CLI, the widget, and the desktop app all go
//! through these functions, so behaviour is identical everywhere.

mod entries;
mod items;
mod marks;
mod preview;
mod stats;
mod timers;
mod views;

pub use entries::{
    adjust_time, adjust_time_with, done, done_with, log, log_count, log_count_with, log_with, undo,
    undo_with,
};
pub use items::{
    ItemChanges, MovePosition, NewItem, add, add_with, edit, edit_with, find_item, move_item,
    move_item_with, remove, remove_with,
};
pub use marks::{mark, mark_with, unmark, unmark_with};
pub use preview::{Preview, preview};
pub use stats::{
    AxisTick, DayStat, DayTotals, ItemStats, PrayerStats, Stats, Totals, stats, time_axis_ticks,
};
pub use timers::{
    StartedTimer, close_expired_timers, start_timer, start_timer_with, start_timer_with_stopped,
    stop_item_timer, stop_item_timer_with, stop_timer, stop_timer_with,
};
pub use views::{
    BlockPlan, DayPlan, DayReport, NextBoundary, Now, PrayerRow, RunningTimer, SleepRow, Today,
    day_plan, history, today,
};

use crate::config::Config;
use chrono::{DateTime, NaiveDate, SubsecRound, Utc};
use salah_domain::day::BlockKey;
use salah_domain::error::Error;
use salah_domain::item::{self, DayItem, DayView, LegacyEntry, LegacyItem, Repeat};
use salah_prayer::schedule::{self, Schedule};
use salah_store::{Store, WriteTx};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
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
fn entry_block(schedule: &Schedule, item: &LegacyItem) -> BlockKey {
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
fn recorded_block(schedule: &Schedule, item: &LegacyItem, date: Option<NaiveDate>) -> BlockKey {
    match date {
        Some(day) => BlockKey {
            day,
            start: item.block,
        },
        None => entry_block(schedule, item),
    }
}

fn report_day(
    items: &[LegacyItem],
    entries: &[LegacyEntry],
    ends: &mut BlockEnds,
    view: DayView,
) -> Vec<DayItem> {
    item::day_items(items, entries, view, |key| ends.end(key))
}

/// Callback that builds the full protocol result while the mutation
/// transaction is still open. It receives the planner result serialized for
/// `applied_ops`, so replay can return the exact response bytes.
pub type ResultRecorder<'a> =
    &'a mut dyn FnMut(&WriteTx<'_>, &str) -> Result<serde_json::Value, Error>;

/// Optional metadata for one mutating use case.
#[derive(Default)]
pub struct Mutation<'a> {
    /// Client idempotency key. Replaying it returns the stored result and
    /// performs no writes.
    pub op_id: Option<&'a str>,
    /// Rejects the mutation with `Error::Conflict` when the target entity's
    /// stored version differs.
    pub expected_version: Option<i64>,
    /// Records the full protocol result in the same transaction as the write.
    pub recorder: Option<ResultRecorder<'a>>,
}

impl<'a> Mutation<'a> {
    pub fn new(op_id: Option<&'a str>, expected_version: Option<i64>) -> Self {
        Self {
            op_id,
            expected_version,
            recorder: None,
        }
    }

    /// Like [`Mutation::new`], but also stores the full protocol result in
    /// `applied_ops` inside the same write transaction.
    pub fn with_result(
        op_id: Option<&'a str>,
        expected_version: Option<i64>,
        recorder: ResultRecorder<'a>,
    ) -> Self {
        Self {
            op_id,
            expected_version,
            recorder: Some(recorder),
        }
    }
}

fn ensure_version(kind: &str, id: &str, actual: i64, expected: Option<i64>) -> Result<(), Error> {
    if let Some(expected) = expected
        && expected != actual
    {
        return Err(Error::Conflict(format!(
            "{kind} {id:?} is at version {actual}, not {expected}"
        )));
    }
    Ok(())
}

/// Results stored for idempotent replay. Entity versions are not part of the
/// CLI JSON, so they are added explicitly here.
trait StoredResult: Serialize + DeserializeOwned {
    fn to_stored_json(&self) -> Result<String, Error>;
    fn from_stored_json(text: &str) -> Result<Self, Error>;
}

#[derive(Serialize, Deserialize)]
struct VersionedItem {
    #[serde(flatten)]
    row: LegacyItem,
    version: i64,
}

#[derive(Serialize, Deserialize)]
struct VersionedEntry {
    #[serde(flatten)]
    row: LegacyEntry,
    version: i64,
}

#[derive(Serialize, Deserialize)]
struct VersionedMark {
    #[serde(flatten)]
    row: salah_domain::item::PrayerMark,
    version: i64,
}

impl StoredResult for LegacyItem {
    fn to_stored_json(&self) -> Result<String, Error> {
        serde_json::to_string(&VersionedItem {
            row: self.clone(),
            version: self.version,
        })
        .map_err(|error| Error::Internal(format!("store: {error}")))
    }

    fn from_stored_json(text: &str) -> Result<Self, Error> {
        let stored: VersionedItem = serde_json::from_str(text).map_err(|error| {
            Error::Storage(format!("store: invalid applied op result: {error}"))
        })?;
        let mut row = stored.row;
        row.version = stored.version;
        Ok(row)
    }
}

impl StoredResult for LegacyEntry {
    fn to_stored_json(&self) -> Result<String, Error> {
        serde_json::to_string(&VersionedEntry {
            row: self.clone(),
            version: self.version,
        })
        .map_err(|error| Error::Internal(format!("store: {error}")))
    }

    fn from_stored_json(text: &str) -> Result<Self, Error> {
        let stored: VersionedEntry = serde_json::from_str(text).map_err(|error| {
            Error::Storage(format!("store: invalid applied op result: {error}"))
        })?;
        let mut row = stored.row;
        row.version = stored.version;
        Ok(row)
    }
}

impl StoredResult for salah_domain::item::PrayerMark {
    fn to_stored_json(&self) -> Result<String, Error> {
        serde_json::to_string(&VersionedMark {
            row: self.clone(),
            version: self.version,
        })
        .map_err(|error| Error::Internal(format!("store: {error}")))
    }

    fn from_stored_json(text: &str) -> Result<Self, Error> {
        let stored: VersionedMark = serde_json::from_str(text).map_err(|error| {
            Error::Storage(format!("store: invalid applied op result: {error}"))
        })?;
        let mut row = stored.row;
        row.version = stored.version;
        Ok(row)
    }
}

impl StoredResult for Vec<LegacyEntry> {
    fn to_stored_json(&self) -> Result<String, Error> {
        let rows: Vec<VersionedEntry> = self
            .iter()
            .map(|row| VersionedEntry {
                row: row.clone(),
                version: row.version,
            })
            .collect();
        serde_json::to_string(&rows).map_err(|error| Error::Internal(format!("store: {error}")))
    }

    fn from_stored_json(text: &str) -> Result<Self, Error> {
        let rows: Vec<VersionedEntry> = serde_json::from_str(text).map_err(|error| {
            Error::Storage(format!("store: invalid applied op result: {error}"))
        })?;
        Ok(rows
            .into_iter()
            .map(|stored| {
                let mut row = stored.row;
                row.version = stored.version;
                row
            })
            .collect())
    }
}

impl StoredResult for usize {
    fn to_stored_json(&self) -> Result<String, Error> {
        serde_json::to_string(self).map_err(|error| Error::Internal(format!("store: {error}")))
    }

    fn from_stored_json(text: &str) -> Result<Self, Error> {
        serde_json::from_str(text)
            .map_err(|error| Error::Storage(format!("store: invalid applied op result: {error}")))
    }
}

/// Runs one planner mutation in a single immediate transaction with
/// idempotency and version preconditions. A replayed
/// `op_id` returns the stored result before any timer repair or validation
/// work, so a retry after a timeout is a true no-op.
fn write_mutation<T: StoredResult>(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    mutation: Mutation<'_>,
    mut work: impl FnMut(&WriteTx<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    let now = stamp(now);
    let Mutation {
        op_id,
        expected_version: _,
        mut recorder,
    } = mutation;
    store.write(|tx| {
        tx.set_op_id(op_id);
        if let Some(op_id) = op_id
            && let Some(applied) = tx.applied_op(op_id)?
        {
            return decode_stored(&applied.result_json);
        }
        timers::close_expired_timers(tx, config, now)?;
        let value = work(tx)?;
        if let Some(op_id) = op_id {
            let result_json = value.to_stored_json()?;
            let stored = match recorder.as_mut() {
                Some(recorder) => {
                    let rpc = recorder(tx, &result_json)?;
                    let result: serde_json::Value =
                        serde_json::from_str(&result_json).map_err(|error| {
                            Error::Internal(format!("serialize applied op result: {error}"))
                        })?;
                    serde_json::to_string(&serde_json::json!({
                        "result": result,
                        "rpc": rpc,
                    }))
                    .map_err(|error| Error::Internal(format!("store: {error}")))?
                }
                None => result_json,
            };
            tx.record_applied_op(op_id, &stored)?;
        }
        Ok(value)
    })
}

/// Reads a planner result from an `applied_ops` row. Newer rows wrap it next
/// to the full protocol result; older rows contain the planner result alone.
fn decode_stored<T: StoredResult>(text: &str) -> Result<T, Error> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| Error::Storage(format!("store: invalid applied op result: {error}")))?;
    match value.get("rpc") {
        Some(_) => match value.get("result") {
            Some(result) => T::from_stored_json(&result.to_string()),
            None => Err(Error::Storage(
                "store: invalid applied op result: missing planner result".into(),
            )),
        },
        None => T::from_stored_json(text),
    }
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
    pub(super) fn ids() -> &'static salah_domain::runtime::SequenceIds {
        static IDS: salah_domain::runtime::SequenceIds = salah_domain::runtime::SequenceIds::new();
        &IDS
    }
}
