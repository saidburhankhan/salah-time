//! The optional reflection saved for one Monday-first week.
use crate::error::Error;
use chrono::{DateTime, Datelike, NaiveDate, Utc, Weekday};
use serde::Serialize;

pub fn validate_week(week: NaiveDate) -> Result<(), Error> {
    if week.weekday() != Weekday::Mon || !(0..=9999).contains(&week.year()) {
        return Err(Error::validation(
            "week",
            "week must be a Monday in YYYY-MM-DD format",
        ));
    }
    crate::goal::Period::Week.bounds(week)?;
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
pub struct Reflection {
    pub id: String,
    pub week: NaiveDate,
    pub note: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub version: i64,
}
