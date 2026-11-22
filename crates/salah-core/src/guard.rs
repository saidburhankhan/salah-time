//! Checks shared by the use cases.

use chrono::{DateTime, SubsecRound, Utc};
use salah_domain::error::Error;

/// Timestamps are stored with millisecond precision; truncating first makes a
/// value read back from the store equal the value that was written.
pub fn stamp(now: DateTime<Utc>) -> DateTime<Utc> {
    now.trunc_subsecs(3)
}

pub(crate) fn not_found(what: &'static str, id: &str) -> Error {
    Error::NotFound {
        what,
        id: id.to_owned(),
    }
}

/// Rejects a write whose `expected_version` no longer matches the entity.
pub(crate) fn ensure_version(
    what: &str,
    id: &str,
    actual: i64,
    expected: Option<i64>,
) -> Result<(), Error> {
    match expected {
        Some(expected) if expected != actual => Err(Error::Conflict(format!(
            "{what} {id:?} is at version {actual}, not {expected}"
        ))),
        _ => Ok(()),
    }
}

/// A short single-line label such as a project, tag, or metric name.
pub(crate) fn label(field: &'static str, value: &str, max: usize) -> Result<String, Error> {
    let value = value.trim();
    if value.is_empty() {
        return Err(Error::validation(
            field,
            format!("{field} must not be empty"),
        ));
    }
    if value.chars().count() > max {
        return Err(Error::validation(
            field,
            format!("{field} must be at most {max} characters"),
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(Error::validation(
            field,
            format!("{field} must not contain control characters"),
        ));
    }
    Ok(value.to_owned())
}

/// A `#rrggbb` colour.
pub(crate) fn color(value: &str) -> Result<String, Error> {
    let value = value.trim();
    let hex = value.strip_prefix('#').unwrap_or("");
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(value.to_ascii_lowercase())
    } else {
        Err(Error::validation("color", "color must look like #4c8bf5"))
    }
}

/// An optional note: trimmed, at most 500 characters, empty means none.
pub(crate) fn note(value: Option<String>) -> Result<Option<String>, Error> {
    let Some(value) = value else { return Ok(None) };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > 500 {
        return Err(Error::validation(
            "note",
            "note must be at most 500 characters",
        ));
    }
    if value.chars().any(|c| c.is_control() && c != '\n') {
        return Err(Error::validation(
            "note",
            "note must not contain control characters",
        ));
    }
    Ok(Some(value.to_owned()))
}

/// Minutes of work: 1 to 1440.
pub(crate) fn minutes(field: &'static str, value: i64) -> Result<i64, Error> {
    if (1..=1440).contains(&value) {
        Ok(value)
    } else {
        Err(Error::validation(
            field,
            format!("{field} must be between 1 and 1440"),
        ))
    }
}

/// Deserializes a present field, `null` included, as `Some`: with
/// `#[serde(default)]` an absent field stays `None`, so `Option<Option<T>>`
/// tells "clear" (`Some(None)`) from "keep" (`None`).
pub(crate) fn nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}
