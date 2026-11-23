//! Period targets and their read-time progress.
use crate::error::Error;
use chrono::{DateTime, Datelike, Days, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// A goal's period: the tracking day (Sleep boundary) or a Monday-first week.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Period {
    Day,
    Week,
}

impl Period {
    /// The first and last day of the period containing `day`.
    pub fn bounds(self, day: NaiveDate) -> Result<(NaiveDate, NaiveDate), Error> {
        let (from, to) = match self {
            Self::Day => (day, day),
            Self::Week => {
                let offset = u64::from(day.weekday().num_days_from_monday());
                let from = day.checked_sub_days(Days::new(offset));
                let to = day.checked_add_days(Days::new(6 - offset));
                (from.ok_or_else(date_error)?, to.ok_or_else(date_error)?)
            }
        };
        if !(0..=9999).contains(&from.year()) || !(0..=9999).contains(&to.year()) {
            return Err(date_error());
        }
        Ok((from, to))
    }
}

fn date_error() -> Error {
    Error::validation("date", "period falls outside YYYY-MM-DD dates")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    AtLeast,
    AtMost,
}

/// At most one id is set; none means all work.
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    pub project_id: Option<String>,
    pub epic_id: Option<String>,
    pub item_id: Option<String>,
}

/// A stored target. Editing a target ends this row and starts a successor
/// (`replaces_id`), so a stored target never changes.
#[derive(Clone, Debug, Serialize)]
pub struct Goal {
    pub id: String,
    #[serde(flatten)]
    pub scope: Scope,
    pub period: Period,
    pub metric_id: Option<String>,
    pub direction: Direction,
    pub target: f64,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
    pub replaces_id: Option<String>,
    pub position: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub version: i64,
}

/// One period's value computed from sessions; `value` is `None` for a metric
/// no session in the period recorded.
#[derive(Clone, Debug, Serialize)]
pub struct Progress {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub value: Option<f64>,
    pub met: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct GoalView {
    #[serde(flatten)]
    pub goal: Goal,
    pub progress: Progress,
}

#[derive(Clone, Debug, Serialize)]
pub struct HistoryRow {
    #[serde(flatten)]
    pub progress: Progress,
    pub target: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn weeks_run_monday_to_sunday_and_days_are_one_day() {
        let week = |day| Period::Week.bounds(d(day)).unwrap();
        assert_eq!(week("2031-05-18"), (d("2031-05-12"), d("2031-05-18")));
        assert_eq!(week("2031-05-19"), (d("2031-05-19"), d("2031-05-25")));
        assert_eq!(week("2031-05-21"), (d("2031-05-19"), d("2031-05-25")));
        let day = Period::Day.bounds(d("2031-05-18")).unwrap();
        assert_eq!(day, (d("2031-05-18"), d("2031-05-18")));
        assert!(Period::Week.bounds(d("9999-12-31")).is_err());
    }
}
