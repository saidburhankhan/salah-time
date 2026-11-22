//! Day-centred tracking entities shared by storage, core, and protocol.

use crate::day::{BlockKey, BoundaryId};
use crate::item::{Repeat, Weekdays};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub color: String,
    pub note: Option<String>,
    pub archived: bool,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(default = "default_version", skip_serializing)]
    pub version: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(default = "default_version", skip_serializing)]
    pub version: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetricKind {
    Count,
    Scale,
    Check,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rollup {
    Sum,
    Avg,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    pub id: String,
    pub name: String,
    pub unit: String,
    pub kind: MetricKind,
    pub scale_max: Option<i64>,
    pub rollup: Rollup,
    pub builtin: bool,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(default = "default_version", skip_serializing)]
    pub version: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectMetric {
    pub project_id: String,
    pub metric_id: String,
    pub required: bool,
    pub position: i64,
}

/// A reusable plan line. `day` is its first/applicable day (the v1
/// `start_day` column); concrete historical plans are represented by
/// [`DayPlanLine`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub project_id: Option<String>,
    pub block: BoundaryId,
    pub repeat: Repeat,
    #[serde(default)]
    pub weekdays: Option<Weekdays>,
    pub day: NaiveDate,
    pub planned_minutes: Option<i64>,
    #[serde(default)]
    pub position: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(default = "default_version", skip_serializing)]
    pub version: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub item_id: Option<String>,
    pub project_id: Option<String>,
    pub block: BlockKey,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub minutes: Option<i64>,
    pub note: Option<String>,
    #[serde(default)]
    pub values: Vec<SessionValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(default = "default_version", skip_serializing)]
    pub version: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionValue {
    pub metric_id: String,
    pub value: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DayBlockSource {
    Official,
    Calculated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayBlock {
    pub day: NaiveDate,
    pub block: BoundaryId,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub source: DayBlockSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayPlanLine {
    pub day: NaiveDate,
    pub block: BoundaryId,
    pub item_id: String,
    pub planned_minutes: Option<i64>,
}

pub fn default_version() -> i64 {
    1
}

impl Metric {
    pub fn validate_value(&self, value: f64) -> Result<(), String> {
        if self.archived {
            return Err(format!("metric {:?} is archived", self.id));
        }
        let valid = match self.kind {
            MetricKind::Count => value >= 0.0 && value.is_finite(),
            MetricKind::Scale => {
                value.fract() == 0.0
                    && value >= 1.0
                    && self.scale_max.is_some_and(|max| value <= max as f64)
            }
            MetricKind::Check => value == 0.0 || value == 1.0,
        };
        if valid {
            Ok(())
        } else {
            Err(format!("invalid value for metric {:?}", self.id))
        }
    }
}

impl Item {
    pub fn is_live(&self) -> bool {
        self.deleted_at.is_none()
    }

    pub fn applies_to(&self, day: NaiveDate) -> bool {
        use chrono::Datelike;
        self.is_live()
            && match self.repeat {
                Repeat::None => self.day == day,
                Repeat::Daily => self.day <= day,
                Repeat::Weekly => {
                    self.day <= day
                        && self
                            .weekdays
                            .is_some_and(|days| days.contains(day.weekday()))
                }
            }
    }
}

impl Session {
    pub fn is_live(&self) -> bool {
        self.deleted_at.is_none()
    }

    pub fn is_running(&self) -> bool {
        self.is_live() && self.started_at.is_some() && self.ended_at.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metric(kind: MetricKind, scale_max: Option<i64>) -> Metric {
        let now = Utc::now();
        Metric {
            id: "metric".into(),
            name: "Metric".into(),
            unit: String::new(),
            kind,
            scale_max,
            rollup: Rollup::Avg,
            builtin: false,
            archived: false,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            version: 1,
        }
    }

    #[test]
    fn metric_values_follow_the_locked_rules() {
        assert!(metric(MetricKind::Count, None).validate_value(0.5).is_ok());
        assert!(
            metric(MetricKind::Count, None)
                .validate_value(-1.0)
                .is_err()
        );
        assert!(
            metric(MetricKind::Scale, Some(5))
                .validate_value(5.0)
                .is_ok()
        );
        assert!(
            metric(MetricKind::Scale, Some(5))
                .validate_value(2.5)
                .is_err()
        );
        assert!(metric(MetricKind::Check, None).validate_value(1.0).is_ok());
        assert!(metric(MetricKind::Check, None).validate_value(2.0).is_err());
    }
}
