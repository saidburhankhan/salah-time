//! A day: its prayer blocks, the items planned in them, the sessions
//! recorded, and how much of the plan was fulfilled.

mod fulfilment;
mod snapshot;
mod view;

pub use fulfilment::{Fulfilment, total_minutes};
pub use snapshot::{Snapshot, block_minutes, day_capacity, freeze, freeze_past, snapshot};
pub use view::{
    BlockPlan, DayPlan, DayRow, NextBoundary, Now, PrayerRow, ProjectRef, SleepRow, Today,
    day_plan, today,
};

use crate::config::Config;
use chrono::{DateTime, NaiveDate, Utc};
use salah_domain::error::Error;
use salah_prayer::schedule;

/// The day in progress at `now`; after sleep it is already the coming day.
pub fn current(config: &Config, now: DateTime<Utc>) -> Result<NaiveDate, Error> {
    Ok(schedule::resolve(config, now)?.day())
}
