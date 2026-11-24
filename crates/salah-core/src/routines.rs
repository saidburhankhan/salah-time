//! A page of daily totals for the app's routine activity strip.

use crate::{Store, goals::parse_day};
use salah_domain::{error::Error, item::RoutineDay};
use serde::Serialize;

const MAX_DAYS: i64 = 14;
const MAX_ROWS: i64 = 500;

#[derive(Serialize)]
pub struct ActivityPage {
    rows: Vec<RoutineDay>,
    next: Option<String>,
}

pub fn activity(
    store: &Store,
    from: &str,
    to: &str,
    limit: Option<i64>,
    before: Option<&str>,
) -> Result<ActivityPage, Error> {
    let from = parse_day("from", from)?;
    let to = parse_day("to", to)?;
    if !(0..MAX_DAYS).contains(&(to - from).num_days()) {
        return Err(Error::validation(
            "to",
            "range must cover between 1 and 14 tracking days",
        ));
    }
    let limit = limit.unwrap_or(MAX_ROWS);
    if !(1..=MAX_ROWS).contains(&limit) {
        return Err(Error::validation(
            "limit",
            "limit must be between 1 and 500",
        ));
    }
    let before = before
        .map(|cursor| {
            let invalid =
                || Error::validation("before", "before must be a previous page's next cursor");
            let (day, id): (String, String) =
                serde_json::from_str(cursor).map_err(|_| invalid())?;
            let day = parse_day("before", &day).map_err(|_| invalid())?;
            if !(from..=to).contains(&day) || id.trim().is_empty() {
                return Err(invalid());
            }
            Ok((day, id))
        })
        .transpose()?;

    let mut rows = store.routine_activity(
        from,
        to,
        before.as_ref().map(|(day, id)| (*day, id.as_str())),
        limit + 1,
    )?;
    let next = if rows.len() > limit as usize {
        rows.truncate(limit as usize);
        rows.last()
            .map(|row| serde_json::to_string(&(row.day, &row.item_id)))
            .transpose()
            .map_err(|error| Error::Internal(error.to_string()))?
    } else {
        None
    };
    Ok(ActivityPage { rows, next })
}
