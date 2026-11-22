use crate::day::BoundaryId;
use crate::error::Error;
use crate::item::{
    Entry, EntryKind, Item, PrayerMark, validate_block, validate_repeat, validate_title,
};
use crate::store::Store;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const FORMAT: &str = "salah-export";
/// Version 2 added prayer marks and item position/weekdays; version 1 files
/// still import because those fields default.
pub const VERSION: u32 = 2;
pub const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Export {
    pub format: String,
    pub version: u32,
    pub exported_at: DateTime<Utc>,
    pub items: Vec<Item>,
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub prayer_marks: Vec<PrayerMark>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct ImportSummary {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
}

pub fn export(store: &Store, now: DateTime<Utc>) -> Result<Export, Error> {
    Ok(Export {
        format: FORMAT.to_string(),
        version: VERSION,
        exported_at: now,
        items: store.items()?,
        entries: store.entries()?,
        prayer_marks: store.marks()?,
    })
}

pub fn parse(bytes: &[u8]) -> Result<Export, Error> {
    if bytes.len() > MAX_BYTES {
        return Err(format!("export exceeds maximum size of {MAX_BYTES} bytes").into());
    }
    let text = std::str::from_utf8(bytes).map_err(|e| format!("export is not valid UTF-8: {e}"))?;
    let data: Export =
        serde_json::from_str(text).map_err(|e| format!("invalid export JSON: {e}"))?;
    validate(&data)?;
    Ok(data)
}

pub fn validate(data: &Export) -> Result<(), Error> {
    if data.format != FORMAT {
        return Err(format!("unsupported export format {:?}", data.format).into());
    }
    if data.version > VERSION {
        return Err(format!(
            "export version {} is newer than this salah (version {VERSION})",
            data.version
        )
        .into());
    }

    let mut item_ids = HashSet::new();
    for item in &data.items {
        validate_id("item", &item.id)?;
        if !item_ids.insert(item.id.as_str()) {
            return Err(format!("duplicate item id {:?}", item.id).into());
        }
        let title = validate_title(&item.title).map_err(|e| format!("item {}: {e}", item.id))?;
        if title != item.title {
            return Err(format!("item {}: title must already be trimmed", item.id).into());
        }
        validate_block(item.block).map_err(|e| format!("item {}: {e}", item.id))?;
        validate_repeat(item.repeat, item.weekdays)
            .map_err(|e| format!("item {}: {e}", item.id))?;
        if item.target.is_some_and(|value| value < 0) {
            return Err(format!("item {}: target must not be negative", item.id).into());
        }
        validate_times("item", &item.id, item.created_at, item.updated_at)?;
    }

    let mut entry_ids = HashSet::new();
    for entry in &data.entries {
        validate_id("entry", &entry.id)?;
        validate_id("entry item", &entry.item_id)?;
        if !entry_ids.insert(entry.id.as_str()) {
            return Err(format!("duplicate entry id {:?}", entry.id).into());
        }
        // Negative values are adjustments that subtract time or count.
        if entry.value.is_some_and(|value| value.abs() > 24 * 60) {
            return Err(format!("entry {}: value is out of range", entry.id).into());
        }
        validate_times("entry", &entry.id, entry.created_at, entry.updated_at)?;
        if entry.kind == EntryKind::Timer && entry.started_at.is_none() {
            return Err(format!("entry {}: timer requires started_at", entry.id).into());
        }
        if let (Some(started), Some(ended)) = (entry.started_at, entry.ended_at)
            && ended < started
        {
            return Err(format!("entry {}: ended_at precedes started_at", entry.id).into());
        }
    }
    let mut mark_ids = HashSet::new();
    for mark in &data.prayer_marks {
        if !mark_ids.insert(mark.id.as_str()) {
            return Err(format!("duplicate prayer mark id {:?}", mark.id).into());
        }
        if mark.prayer == BoundaryId::Sleep || mark.id != PrayerMark::id_for(mark.day, mark.prayer)
        {
            return Err(format!("prayer mark {:?} is invalid", mark.id).into());
        }
        validate_times("prayer mark", &mark.id, mark.created_at, mark.updated_at)?;
    }
    Ok(())
}

pub fn import(store: &mut Store, data: &Export) -> Result<ImportSummary, Error> {
    validate(data)?;
    store.transaction(|store| {
        let exported_items: HashSet<&str> =
            data.items.iter().map(|item| item.id.as_str()).collect();
        for entry in &data.entries {
            if !exported_items.contains(entry.item_id.as_str())
                && store.item(&entry.item_id)?.is_none()
            {
                return Err(format!(
                    "entry {} references missing item {}",
                    entry.id, entry.item_id
                )
                .into());
            }
        }

        let mut summary = ImportSummary::default();
        for item in &data.items {
            match store.item(&item.id)? {
                None => {
                    store.put_item(item)?;
                    summary.added += 1;
                }
                Some(current) if item.updated_at > current.updated_at => {
                    store.put_item(item)?;
                    summary.updated += 1;
                }
                Some(_) => summary.unchanged += 1,
            }
        }
        for entry in &data.entries {
            match store.entry(&entry.id)? {
                None => {
                    store.put_entry(entry)?;
                    summary.added += 1;
                }
                Some(current) if entry.updated_at > current.updated_at => {
                    store.put_entry(entry)?;
                    summary.updated += 1;
                }
                Some(_) => summary.unchanged += 1,
            }
        }
        for mark in &data.prayer_marks {
            match store.mark(&mark.id)? {
                None => {
                    store.put_mark(mark)?;
                    summary.added += 1;
                }
                Some(current) if mark.updated_at > current.updated_at => {
                    store.put_mark(mark)?;
                    summary.updated += 1;
                }
                Some(_) => summary.unchanged += 1,
            }
        }
        Ok(summary)
    })
}

fn validate_id(kind: &str, id: &str) -> Result<(), Error> {
    let length = id.chars().count();
    if length == 0 {
        return Err(format!("{kind} id must not be empty").into());
    }
    if length > 64 {
        return Err(format!("{kind} id must be at most 64 characters").into());
    }
    if id.chars().any(char::is_control) {
        return Err(format!("{kind} id must not contain control characters").into());
    }
    Ok(())
}

fn validate_times(
    kind: &str,
    id: &str,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> Result<(), Error> {
    if updated_at < created_at {
        return Err(format!("{kind} {id}: updated_at precedes created_at").into());
    }
    Ok(())
}
