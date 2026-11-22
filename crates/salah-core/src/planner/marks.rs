use super::{stamp, write};
use crate::config::Config;
use chrono::{DateTime, NaiveDate, Utc};
use salah_domain::day::BoundaryId;
use salah_domain::error::Error;
use salah_domain::item::PrayerMark;
use salah_prayer::schedule;
use salah_store::Store;

pub(super) fn live_marks(store: &Store, day: NaiveDate) -> Result<Vec<BoundaryId>, Error> {
    let mut marks: Vec<BoundaryId> = store
        .marks_between(day, day)?
        .into_iter()
        .filter(PrayerMark::is_live)
        .map(|m| m.prayer)
        .collect();
    marks.sort();
    Ok(marks)
}

fn mark_prayer(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    prayer: BoundaryId,
    date: Option<NaiveDate>,
    prayed: bool,
) -> Result<PrayerMark, Error> {
    if !prayer.is_prayer() {
        return Err(format!("{} is not a prayer", prayer.name()).into());
    }
    let now = stamp(now);
    let day = match date {
        Some(day) => day,
        None => schedule::resolve(config, now)?.day(),
    };
    let id = PrayerMark::id_for(day, prayer);
    write(store, config, now, |tx| {
        let mut mark = tx.mark(&id)?.unwrap_or(PrayerMark {
            id: id.clone(),
            day,
            prayer,
            marked_at: now,
            created_at: now,
            updated_at: now,
            deleted_at: Some(now),
        });
        // Repeating the current state changes nothing, so marking twice is safe.
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
        tx.put_mark(&mark)?;
        Ok(mark)
    })
}

/// Marks a prayer as prayed on `date` (default: the current day).
pub fn mark(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    prayer: BoundaryId,
    date: Option<NaiveDate>,
) -> Result<PrayerMark, Error> {
    mark_prayer(store, config, now, prayer, date, true)
}

/// Removes a prayer's mark on `date` (default: the current day).
pub fn unmark(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    prayer: BoundaryId,
    date: Option<NaiveDate>,
) -> Result<PrayerMark, Error> {
    mark_prayer(store, config, now, prayer, date, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::test_support::{at, config, date};
    use crate::planner::{day_plan, today};

    #[test]
    fn prayers_can_be_marked_and_unmarked_idempotently() {
        let mut store = Store::open_in_memory().unwrap();
        mark(
            &mut store,
            &config(),
            at(20, 13, 0),
            BoundaryId::Dhuhr,
            None,
        )
        .unwrap();
        mark(
            &mut store,
            &config(),
            at(20, 13, 1),
            BoundaryId::Dhuhr,
            None,
        )
        .unwrap();
        mark(
            &mut store,
            &config(),
            at(20, 13, 2),
            BoundaryId::Fajr,
            Some(date(19)),
        )
        .unwrap();
        assert_eq!(store.marks().unwrap().len(), 2);
        let plan = day_plan(&store, &config(), at(20, 13, 3), None).unwrap();
        let marked: Vec<_> = plan
            .prayers
            .iter()
            .filter(|p| p.marked)
            .map(|p| p.id)
            .collect();
        assert_eq!(marked, [BoundaryId::Dhuhr]);
        unmark(
            &mut store,
            &config(),
            at(20, 13, 4),
            BoundaryId::Dhuhr,
            None,
        )
        .unwrap();
        assert!(
            today(&store, &config(), at(20, 13, 5))
                .unwrap()
                .prayer_marks
                .is_empty()
        );
        assert!(
            mark(
                &mut store,
                &config(),
                at(20, 13, 6),
                BoundaryId::Sleep,
                None
            )
            .is_err()
        );
    }
}
