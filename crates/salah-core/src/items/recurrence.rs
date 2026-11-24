//! Apply schedule changes to the current item after its prior rule is preserved.

use super::{ItemChanges, one_time};
use crate::{Store, goals::parse_day};
use chrono::NaiveDate;
use salah_domain::{
    error::Error,
    item::{Repeat, validate_repeat},
    tracking::Item,
};

pub(super) fn edit(
    store: &Store,
    item: &mut Item,
    changes: &ItemChanges,
    today: NaiveDate,
) -> Result<(), Error> {
    let repeat = changes.repeat.unwrap_or(item.repeat);
    if item.repeat != Repeat::None && repeat == Repeat::None {
        let date = changes
            .date
            .as_deref()
            .map(|s| parse_day("date", s))
            .transpose()?
            .unwrap_or(today);
        if date < today || store.day_written(date)? {
            return Err(Error::validation(
                "date",
                "choose an unfrozen date from today onward",
            ));
        }
        item.day = Some(date);
        item.end_day = None;
    } else if changes.date.is_some() {
        return Err(Error::validation(
            "date",
            "date is for converting a routine to a one-time task",
        ));
    }
    if item.is_backlog() {
        one_time(repeat)?;
    }
    if repeat != item.repeat && repeat != Repeat::Weekly {
        item.weekdays = None;
    }
    item.repeat = repeat;
    if let Some(days) = changes.weekdays {
        item.weekdays = Some(days);
    }
    validate_repeat(repeat, item.weekdays)?;
    if let Some(end) = &changes.end_day {
        item.end_day = end
            .as_deref()
            .map(|value| parse_day("end_day", value))
            .transpose()?;
        if let Some(last) = item.end_day {
            if repeat == Repeat::None {
                return Err(Error::validation(
                    "end_day",
                    "only routines have an end day",
                ));
            }
            if item.day.is_none_or(|first| last < first) {
                return Err(Error::validation(
                    "end_day",
                    "the end day must be on or after the routine's first day",
                ));
            }
        }
    }
    Ok(())
}
