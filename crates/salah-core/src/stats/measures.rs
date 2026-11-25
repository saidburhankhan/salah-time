//! The session predicates `stats.get`, `stats.table` and `sessions.list`
//! share: a duration range and a value range per metric. They narrow which
//! finished sessions a report aggregates or lists; planned minutes, capacity
//! and the fulfilment denominator never depend on them.

use crate::{Store, metrics};
use salah_domain::error::Error;
use salah_store::reports::{Bounds, SessionFilter};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Inclusive bounds on one metric; a session lacking the value does not match.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ValueRange {
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
}

/// Duration and metric bounds on sessions. Empty means no narrowing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Measures {
    #[serde(default)]
    pub minutes_min: Option<i64>,
    #[serde(default)]
    pub minutes_max: Option<i64>,
    #[serde(default)]
    pub values: BTreeMap<String, ValueRange>,
}

impl Measures {
    /// The store predicate. Negative minutes, a reversed range and a metric
    /// that does not exist are rejected.
    pub(crate) fn resolve(&self, store: &Store) -> Result<SessionFilter<'_>, Error> {
        for (field, minutes) in [
            ("minutes_min", self.minutes_min),
            ("minutes_max", self.minutes_max),
        ] {
            if minutes.is_some_and(|m| m < 0) {
                return Err(Error::validation(field, "minutes cannot be negative"));
            }
        }
        if let (Some(min), Some(max)) = (self.minutes_min, self.minutes_max)
            && min > max
        {
            return Err(Error::validation(
                "minutes_max",
                "minutes_max is below minutes_min",
            ));
        }
        let mut values = BTreeMap::new();
        for (metric_id, range) in &self.values {
            if metric_id != "distractions_per_hour" {
                metrics::find(store, metric_id)?;
            }
            if [range.min, range.max]
                .into_iter()
                .flatten()
                .any(|bound| !bound.is_finite())
            {
                return Err(Error::validation(
                    "values",
                    format!("the {metric_id} bounds must be finite"),
                ));
            }
            if let (Some(min), Some(max)) = (range.min, range.max)
                && min > max
            {
                return Err(Error::validation(
                    "values",
                    format!("the {metric_id} range ends below its start"),
                ));
            }
            values.insert(
                metric_id.as_str(),
                Bounds {
                    min: range.min,
                    max: range.max,
                },
            );
        }
        Ok(SessionFilter {
            minutes_min: self.minutes_min,
            minutes_max: self.minutes_max,
            values,
        })
    }
}
