use crate::item::{Entry, EntryKind, Item, validate_block, validate_title};
use crate::store::Store;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const FORMAT: &str = "salah-export";
pub const VERSION: u32 = 1;
pub const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Export {
    pub format: String,
    pub version: u32,
    pub exported_at: DateTime<Utc>,
    pub items: Vec<Item>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct ImportSummary {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
}

pub fn export(store: &Store, now: DateTime<Utc>) -> Result<Export, String> {
    Ok(Export {
        format: FORMAT.to_string(),
        version: VERSION,
        exported_at: now,
        items: store.items()?,
        entries: store.entries()?,
    })
}

pub fn parse(bytes: &[u8]) -> Result<Export, String> {
    if bytes.len() > MAX_BYTES {
        return Err(format!("export exceeds maximum size of {MAX_BYTES} bytes"));
    }
    let text = std::str::from_utf8(bytes).map_err(|e| format!("export is not valid UTF-8: {e}"))?;
    let data: Export =
        serde_json::from_str(text).map_err(|e| format!("invalid export JSON: {e}"))?;
    validate(&data)?;
    Ok(data)
}

pub fn validate(data: &Export) -> Result<(), String> {
    if data.format != FORMAT {
        return Err(format!("unsupported export format {:?}", data.format));
    }
    if data.version > VERSION {
        return Err(format!(
            "export version {} is newer than this salah (version {VERSION})",
            data.version
        ));
    }

    let mut item_ids = HashSet::new();
    for item in &data.items {
        validate_id("item", &item.id)?;
        if !item_ids.insert(item.id.as_str()) {
            return Err(format!("duplicate item id {:?}", item.id));
        }
        let title = validate_title(&item.title).map_err(|e| format!("item {}: {e}", item.id))?;
        if title != item.title {
            return Err(format!("item {}: title must already be trimmed", item.id));
        }
        validate_block(item.block).map_err(|e| format!("item {}: {e}", item.id))?;
        if item.target.is_some_and(|value| value < 0) {
            return Err(format!("item {}: target must not be negative", item.id));
        }
        validate_times("item", &item.id, item.created_at, item.updated_at)?;
    }

    let mut entry_ids = HashSet::new();
    for entry in &data.entries {
        validate_id("entry", &entry.id)?;
        validate_id("entry item", &entry.item_id)?;
        if !entry_ids.insert(entry.id.as_str()) {
            return Err(format!("duplicate entry id {:?}", entry.id));
        }
        if entry.value.is_some_and(|value| value < 0) {
            return Err(format!("entry {}: value must not be negative", entry.id));
        }
        validate_times("entry", &entry.id, entry.created_at, entry.updated_at)?;
        if entry.kind == EntryKind::Timer && entry.started_at.is_none() {
            return Err(format!("entry {}: timer requires started_at", entry.id));
        }
        if let (Some(started), Some(ended)) = (entry.started_at, entry.ended_at)
            && ended < started
        {
            return Err(format!("entry {}: ended_at precedes started_at", entry.id));
        }
    }
    Ok(())
}

pub fn import(store: &mut Store, data: &Export) -> Result<ImportSummary, String> {
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
                ));
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
        Ok(summary)
    })
}

fn validate_id(kind: &str, id: &str) -> Result<(), String> {
    let length = id.chars().count();
    if length == 0 {
        return Err(format!("{kind} id must not be empty"));
    }
    if length > 64 {
        return Err(format!("{kind} id must be at most 64 characters"));
    }
    if id.chars().any(char::is_control) {
        return Err(format!("{kind} id must not contain control characters"));
    }
    Ok(())
}

fn validate_times(
    kind: &str,
    id: &str,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> Result<(), String> {
    if updated_at < created_at {
        return Err(format!("{kind} {id}: updated_at precedes created_at"));
    }
    Ok(())
}
