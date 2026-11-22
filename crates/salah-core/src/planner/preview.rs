//! Read-only drafts for the entry form: no store access and no writes.

use chrono::{Datelike, NaiveDate};
use salah_domain::day::BoundaryId;
use salah_domain::error::Error;
use salah_domain::item::{
    Measure, Repeat, Weekdays, next_occurrence, schedule_text, validate_block, validate_repeat,
};
use serde::Serialize;

/// The recurrence preview shown while a user edits a draft.
#[derive(Clone, Debug, Serialize)]
pub struct Preview {
    /// First date on or after the draft date on which the draft applies.
    pub next_occurrence: NaiveDate,
    /// Remaining occurrences in the draft date's calendar month, from the
    /// draft date onwards.
    pub occurrences_this_month: usize,
    /// Locale-neutral English schedule label.
    pub schedule_text: String,
}

/// Computes a draft preview without reading or writing the store.
pub fn preview(
    block: BoundaryId,
    repeat: Repeat,
    weekdays: Option<Weekdays>,
    measure: Measure,
    target: Option<i64>,
    date: NaiveDate,
) -> Result<Preview, Error> {
    validate_block(block)?;
    validate_repeat(repeat, weekdays)?;
    if let Some(target) = target {
        if measure == Measure::Check {
            return Err("a target needs a minutes or count item".into());
        }
        if target <= 0 {
            return Err("target must be positive".into());
        }
    }
    let next = next_occurrence(repeat, weekdays, date).ok_or_else(|| Error::Validation {
        field: "weekdays",
        message: "a weekly item needs at least one weekday".into(),
    })?;
    Ok(Preview {
        next_occurrence: next,
        occurrences_this_month: occurrences_from(repeat, weekdays, date),
        schedule_text: schedule_text(repeat, weekdays),
    })
}

/// Counts applicable dates from `from` through the end of its month.
fn occurrences_from(repeat: Repeat, weekdays: Option<Weekdays>, from: NaiveDate) -> usize {
    if repeat == Repeat::None {
        return 1;
    }
    let last = month_end(from);
    let mut day = from;
    let mut count = 0;
    while day <= last {
        if next_occurrence(repeat, weekdays, day) == Some(day) {
            count += 1;
        }
        let Some(next) = day.succ_opt() else {
            break;
        };
        day = next;
    }
    count
}

fn month_end(date: NaiveDate) -> NaiveDate {
    let (year, month) = if date.month() == 12 {
        (date.year() + 1, 1)
    } else {
        (date.year(), date.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1)
        .and_then(|first| first.pred_opt())
        .unwrap_or(date)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn preview_for(repeat: Repeat, weekdays: Option<Weekdays>, day: NaiveDate) -> Preview {
        preview(
            BoundaryId::Fajr,
            repeat,
            weekdays,
            Measure::Check,
            None,
            day,
        )
        .unwrap()
    }

    #[test]
    fn preview_counts_the_remaining_days_of_the_month() {
        // 2026-11-20 is a Friday; daily drafts count through 30 November.
        let daily = preview_for(Repeat::Daily, None, date(2026, 11, 20));
        assert_eq!(daily.next_occurrence, date(2026, 11, 20));
        assert_eq!(daily.occurrences_this_month, 11);
        assert_eq!(daily.schedule_text, "Daily");
    }

    #[test]
    fn preview_counts_weekly_occurrences_within_the_month() {
        // Mon/Wed/Fri from Friday 20 November: 20, 23, 25, 27, 30.
        let weekly = preview_for(
            Repeat::Weekly,
            Some(Weekdays::parse_list("mon,wed,fri").unwrap()),
            date(2026, 11, 20),
        );
        assert_eq!(weekly.next_occurrence, date(2026, 11, 20));
        assert_eq!(weekly.occurrences_this_month, 5);
        assert_eq!(weekly.schedule_text, "Mon, Wed, Fri");
    }

    #[test]
    fn preview_crosses_into_the_next_month_when_needed() {
        // A Monday-only draft on 31 December 2026 starts on 4 January 2027.
        let weekly = preview_for(
            Repeat::Weekly,
            Some(Weekdays::parse_list("mon").unwrap()),
            date(2026, 12, 31),
        );
        assert_eq!(weekly.next_occurrence, date(2027, 1, 4));
        assert_eq!(weekly.occurrences_this_month, 0);
    }

    #[test]
    fn preview_validates_the_draft() {
        assert!(
            preview(
                BoundaryId::Sleep,
                Repeat::Daily,
                None,
                Measure::Check,
                None,
                date(2026, 11, 20)
            )
            .is_err()
        );
        assert!(
            preview(
                BoundaryId::Fajr,
                Repeat::Weekly,
                None,
                Measure::Check,
                None,
                date(2026, 11, 20)
            )
            .is_err()
        );
    }
}
