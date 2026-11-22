//! Conversions between domain values and their SQLite text/integer columns,
//! plus the row mappers shared by every query.

use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use rusqlite::{Row, types::Type};
use salah_domain::{
    day::BlockKey,
    error::Error,
    item::{PrayerMark, Weekdays},
    tracking::{DayBlock, DayPlanLine, Item, Metric, Project, ProjectMetric, Session, Tag},
};
use serde::{Serialize, de::DeserializeOwned};

/// Maps a SQLite failure to the domain error class: lock contention is
/// retryable `Busy`, anything else is `Storage`.
pub(crate) fn storage_error(error: rusqlite::Error) -> Error {
    let message = format!("store: {error}");
    let busy = matches!(
        &error,
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
    );
    if busy {
        Error::Busy(message)
    } else {
        Error::Storage(message)
    }
}

pub(crate) fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub(crate) fn day(value: NaiveDate) -> String {
    value.format("%Y-%m-%d").to_string()
}

/// A unit enum stored as its serde name (`"fajr"`, `"weekly"`, `"scale"`, …).
pub(crate) fn name<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => name,
        other => unreachable!("enum column must serialize as a string, got {other:?}"),
    }
}

fn invalid(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error))
}

fn get_timestamp(row: &Row, column: &str) -> rusqlite::Result<DateTime<Utc>> {
    let text: String = row.get(column)?;
    DateTime::parse_from_rfc3339(&text)
        .map(|value| value.with_timezone(&Utc))
        .map_err(invalid)
}

fn get_optional_timestamp(row: &Row, column: &str) -> rusqlite::Result<Option<DateTime<Utc>>> {
    let text: Option<String> = row.get(column)?;
    text.map(|text| {
        DateTime::parse_from_rfc3339(&text)
            .map(|value| value.with_timezone(&Utc))
            .map_err(invalid)
    })
    .transpose()
}

fn get_day(row: &Row, column: &str) -> rusqlite::Result<NaiveDate> {
    let text: String = row.get(column)?;
    NaiveDate::parse_from_str(&text, "%Y-%m-%d").map_err(invalid)
}

fn get_name<T: DeserializeOwned>(row: &Row, column: &str) -> rusqlite::Result<T> {
    let text: String = row.get(column)?;
    serde_json::from_value(serde_json::Value::String(text)).map_err(invalid)
}

pub(crate) fn project(row: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get("id")?,
        name: row.get("name")?,
        color: row.get("color")?,
        note: row.get("note")?,
        archived: row.get("archived")?,
        tag_ids: Vec::new(),
        created_at: get_timestamp(row, "created_at")?,
        updated_at: get_timestamp(row, "updated_at")?,
        deleted_at: get_optional_timestamp(row, "deleted_at")?,
        version: row.get("version")?,
    })
}

pub(crate) fn tag(row: &Row) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: row.get("id")?,
        name: row.get("name")?,
        color: row.get("color")?,
        created_at: get_timestamp(row, "created_at")?,
        updated_at: get_timestamp(row, "updated_at")?,
        deleted_at: get_optional_timestamp(row, "deleted_at")?,
        version: row.get("version")?,
    })
}

pub(crate) fn metric(row: &Row) -> rusqlite::Result<Metric> {
    Ok(Metric {
        id: row.get("id")?,
        name: row.get("name")?,
        unit: row.get("unit")?,
        kind: get_name(row, "kind")?,
        scale_max: row.get("scale_max")?,
        rollup: get_name(row, "rollup")?,
        builtin: row.get("builtin")?,
        archived: row.get("archived")?,
        created_at: get_timestamp(row, "created_at")?,
        updated_at: get_timestamp(row, "updated_at")?,
        deleted_at: get_optional_timestamp(row, "deleted_at")?,
        version: row.get("version")?,
    })
}

pub(crate) fn project_metric(row: &Row) -> rusqlite::Result<ProjectMetric> {
    Ok(ProjectMetric {
        project_id: row.get("project_id")?,
        metric_id: row.get("metric_id")?,
        required: row.get("required")?,
        position: row.get("position")?,
    })
}

pub(crate) fn item(row: &Row) -> rusqlite::Result<Item> {
    Ok(Item {
        id: row.get("id")?,
        title: row.get("title")?,
        project_id: row.get("project_id")?,
        block: get_name(row, "block")?,
        repeat: get_name(row, "repeat")?,
        weekdays: row.get::<_, Option<u8>>("weekdays")?.map(Weekdays),
        day: get_day(row, "start_day")?,
        planned_minutes: row.get("planned_minutes")?,
        position: row.get("position")?,
        created_at: get_timestamp(row, "created_at")?,
        updated_at: get_timestamp(row, "updated_at")?,
        deleted_at: get_optional_timestamp(row, "deleted_at")?,
        version: row.get("version")?,
    })
}

/// A session row without its metric values, which live in their own table.
pub(crate) fn session(row: &Row) -> rusqlite::Result<Session> {
    Ok(Session {
        id: row.get("id")?,
        item_id: row.get("item_id")?,
        project_id: row.get("project_id")?,
        block: BlockKey {
            day: get_day(row, "day")?,
            start: get_name(row, "block")?,
        },
        started_at: get_optional_timestamp(row, "started_at")?,
        ended_at: get_optional_timestamp(row, "ended_at")?,
        minutes: row.get("minutes")?,
        note: row.get("note")?,
        values: Vec::new(),
        created_at: get_timestamp(row, "created_at")?,
        updated_at: get_timestamp(row, "updated_at")?,
        deleted_at: get_optional_timestamp(row, "deleted_at")?,
        version: row.get("version")?,
    })
}

pub(crate) fn day_block(row: &Row) -> rusqlite::Result<DayBlock> {
    Ok(DayBlock {
        day: get_day(row, "day")?,
        block: get_name(row, "block")?,
        starts_at: get_timestamp(row, "starts_at")?,
        ends_at: get_timestamp(row, "ends_at")?,
        source: get_name(row, "source")?,
    })
}

pub(crate) fn day_plan_line(row: &Row) -> rusqlite::Result<DayPlanLine> {
    Ok(DayPlanLine {
        day: get_day(row, "day")?,
        block: get_name(row, "block")?,
        item_id: row.get("item_id")?,
        planned_minutes: row.get("planned_minutes")?,
    })
}

pub(crate) fn mark(row: &Row) -> rusqlite::Result<PrayerMark> {
    Ok(PrayerMark {
        id: row.get("id")?,
        day: get_day(row, "day")?,
        prayer: get_name(row, "prayer")?,
        marked_at: get_timestamp(row, "marked_at")?,
        created_at: get_timestamp(row, "created_at")?,
        updated_at: get_timestamp(row, "updated_at")?,
        deleted_at: get_optional_timestamp(row, "deleted_at")?,
        version: row.get("version")?,
    })
}

pub(crate) fn change(row: &Row) -> rusqlite::Result<crate::Change> {
    Ok(crate::Change {
        seq: row.get("seq")?,
        revision: row.get("revision")?,
        entity: row.get("entity")?,
        entity_id: row.get("entity_id")?,
        op: row.get("op")?,
        version: row.get("version")?,
        at: get_timestamp(row, "at")?,
        op_id: row.get("op_id")?,
    })
}

pub(crate) fn applied_op(row: &Row) -> rusqlite::Result<crate::AppliedOp> {
    Ok(crate::AppliedOp {
        revision: row.get("revision")?,
        result_json: row.get("result_json")?,
        at: get_timestamp(row, "at")?,
    })
}
