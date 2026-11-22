//! Metrics: what a session can be rated or counted by.

use crate::guard::{ensure_version, label, not_found};
use crate::{Store, WriteTx};
use chrono::{DateTime, Utc};
use salah_domain::{
    error::Error,
    runtime::IdGenerator,
    tracking::{Metric, MetricKind, Rollup},
};
use serde::Deserialize;

const NAME_MAX: usize = 60;
const UNIT_MAX: usize = 20;
const SCALE_MAX: i64 = 10;

#[derive(Clone, Debug, Deserialize)]
pub struct NewMetric {
    pub name: String,
    #[serde(default)]
    pub unit: String,
    pub kind: MetricKind,
    #[serde(default)]
    pub scale_max: Option<i64>,
    /// Defaults to `avg` for a scale and `sum` otherwise.
    #[serde(default)]
    pub rollup: Option<Rollup>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MetricChanges {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub archived: Option<bool>,
}

/// Live metrics, built-ins first; `archived` filters by archive state.
pub fn list(store: &Store, archived: Option<bool>) -> Result<Vec<Metric>, Error> {
    Ok(store
        .metrics()?
        .into_iter()
        .filter(|m| m.deleted_at.is_none() && archived.is_none_or(|a| m.archived == a))
        .collect())
}

/// A live metric, archived or not.
pub fn find(store: &Store, id: &str) -> Result<Metric, Error> {
    store
        .metric(id)?
        .filter(|m| m.deleted_at.is_none())
        .ok_or_else(|| not_found("metric", id))
}

pub fn add(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    new: NewMetric,
) -> Result<Metric, Error> {
    let scale_max = match (new.kind, new.scale_max) {
        (MetricKind::Scale, Some(max)) if (2..=SCALE_MAX).contains(&max) => Some(max),
        (MetricKind::Scale, _) => {
            return Err(Error::validation(
                "scale_max",
                format!("a scale needs scale_max between 2 and {SCALE_MAX}"),
            ));
        }
        (_, None) => None,
        (_, Some(_)) => {
            return Err(Error::validation("scale_max", "only a scale has scale_max"));
        }
    };
    let default_rollup = match new.kind {
        MetricKind::Scale => Rollup::Avg,
        MetricKind::Count | MetricKind::Check => Rollup::Sum,
    };
    let mut metric = Metric {
        id: ids.new_id(),
        name: label("name", &new.name, NAME_MAX)?,
        unit: unit(&new.unit)?,
        kind: new.kind,
        scale_max,
        rollup: new.rollup.unwrap_or(default_rollup),
        builtin: false,
        archived: false,
        created_at: now,
        updated_at: now,
        deleted_at: None,
        version: 1,
    };
    metric.version = tx.put_metric(&metric)?;
    Ok(metric)
}

pub fn edit(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    id: &str,
    changes: MetricChanges,
    expected_version: Option<i64>,
) -> Result<Metric, Error> {
    let mut metric = find(tx, id)?;
    ensure_version("metric", id, metric.version, expected_version)?;
    if let Some(name) = &changes.name {
        metric.name = label("name", name, NAME_MAX)?;
    }
    if let Some(value) = &changes.unit {
        metric.unit = unit(value)?;
    }
    if let Some(archived) = changes.archived {
        if archived && metric.builtin {
            return Err(Error::validation(
                "archived",
                "a built-in metric cannot be archived",
            ));
        }
        metric.archived = archived;
    }
    metric.updated_at = now;
    metric.version = tx.put_metric(&metric)?;
    Ok(metric)
}

/// A unit may be empty (a plain scale) but is otherwise a short label.
fn unit(value: &str) -> Result<String, Error> {
    if value.trim().is_empty() {
        Ok(String::new())
    } else {
        label("unit", value, UNIT_MAX)
    }
}
