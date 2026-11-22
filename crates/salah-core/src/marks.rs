//! Prayer marks: which prayers were prayed on a day.

use crate::config::Config;
use crate::guard::ensure_version;
use crate::{Store, WriteTx};
use chrono::{DateTime, NaiveDate, Utc};
use salah_domain::{day::BoundaryId, error::Error, item::PrayerMark};
use salah_prayer::schedule;

/// The prayers marked on `day`, in prayer order.
pub fn live_marks(store: &Store, day: NaiveDate) -> Result<Vec<BoundaryId>, Error> {
    let mut marks: Vec<BoundaryId> = store
        .marks_between(day, day)?
        .into_iter()
        .filter(PrayerMark::is_live)
        .map(|mark| mark.prayer)
        .collect();
    marks.sort();
    Ok(marks)
}

/// Marks (`prayed`) or unmarks a prayer on `date` (default: the day in
/// progress). Repeating the current state writes nothing.
pub fn set(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    prayer: BoundaryId,
    date: Option<NaiveDate>,
    prayed: bool,
    expected_version: Option<i64>,
) -> Result<PrayerMark, Error> {
    if !prayer.is_prayer() {
        return Err(Error::validation(
            "prayer",
            format!("{} is not a prayer", prayer.name()),
        ));
    }
    let day = match date {
        Some(day) => day,
        None => schedule::resolve(config, now)?.day(),
    };
    let id = PrayerMark::id_for(day, prayer);
    let mut mark = tx.mark(&id)?.unwrap_or(PrayerMark {
        id: id.clone(),
        day,
        prayer,
        marked_at: now,
        created_at: now,
        updated_at: now,
        deleted_at: Some(now),
        version: 0,
    });
    if mark.version > 0 {
        ensure_version("prayer mark", &id, mark.version, expected_version)?;
    }
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
    mark.version = tx.put_mark(&mark)?;
    Ok(mark)
}
