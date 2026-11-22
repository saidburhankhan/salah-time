//! Tags group projects for filtering statistics.

use crate::guard::{color, ensure_version, label, not_found};
use crate::{Store, WriteTx};
use chrono::{DateTime, Utc};
use salah_domain::{error::Error, runtime::IdGenerator, tracking::Tag};

const NAME_MAX: usize = 40;

/// Live tags by name.
pub fn list(store: &Store) -> Result<Vec<Tag>, Error> {
    Ok(store
        .tags()?
        .into_iter()
        .filter(|t| t.deleted_at.is_none())
        .collect())
}

pub fn find(store: &Store, id: &str) -> Result<Tag, Error> {
    store
        .tag(id)?
        .filter(|t| t.deleted_at.is_none())
        .ok_or_else(|| not_found("tag", id))
}

/// Adds a tag. Names are unique, so re-adding the name of a removed tag
/// restores that tag with the new colour.
pub fn add(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    name: &str,
    tag_color: &str,
) -> Result<Tag, Error> {
    let name = label("name", name, NAME_MAX)?;
    let tag_color = color(tag_color)?;
    let mut tag = match tx.tags()?.into_iter().find(|t| t.name == name) {
        Some(t) if t.deleted_at.is_none() => {
            return Err(Error::validation(
                "name",
                format!("a tag named {name:?} already exists"),
            ));
        }
        Some(removed) => Tag {
            color: tag_color,
            updated_at: now,
            deleted_at: None,
            ..removed
        },
        None => Tag {
            id: ids.new_id(),
            name,
            color: tag_color,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            version: 1,
        },
    };
    tag.version = tx.put_tag(&tag)?;
    Ok(tag)
}

pub fn edit(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    id: &str,
    name: Option<&str>,
    tag_color: Option<&str>,
    expected_version: Option<i64>,
) -> Result<Tag, Error> {
    let mut tag = find(tx, id)?;
    ensure_version("tag", id, tag.version, expected_version)?;
    if let Some(name) = name {
        let name = label("name", name, NAME_MAX)?;
        match tx.tags()?.iter().find(|t| t.name == name && t.id != tag.id) {
            Some(other) if other.deleted_at.is_some() => {
                return Err(Error::validation(
                    "name",
                    format!("a removed tag is named {name:?}; add it again to restore it"),
                ));
            }
            Some(_) => {
                return Err(Error::validation(
                    "name",
                    format!("a tag named {name:?} already exists"),
                ));
            }
            None => {}
        }
        tag.name = name;
    }
    if let Some(value) = tag_color {
        tag.color = color(value)?;
    }
    tag.updated_at = now;
    tag.version = tx.put_tag(&tag)?;
    Ok(tag)
}

/// Removes a tag and detaches it from every project.
pub fn remove(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    id: &str,
    expected_version: Option<i64>,
) -> Result<Tag, Error> {
    let mut tag = find(tx, id)?;
    ensure_version("tag", id, tag.version, expected_version)?;
    for mut project in tx.projects()? {
        if project.tag_ids.contains(&tag.id) {
            project.tag_ids.retain(|t| *t != tag.id);
            project.updated_at = now;
            tx.put_project(&project)?;
        }
    }
    tag.deleted_at = Some(now);
    tag.updated_at = now;
    tag.version = tx.put_tag(&tag)?;
    Ok(tag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{at, ids};

    #[test]
    fn renaming_onto_a_removed_tag_says_how_to_restore_it() {
        let mut store = Store::open_in_memory().unwrap();
        let (old, other) = store
            .write(|tx| {
                let old = add(tx, at(20, 9, 0), ids(), "Deen", "#00aa00")?;
                let other = add(tx, at(20, 9, 0), ids(), "Work", "#0000aa")?;
                remove(tx, at(20, 9, 1), &old.id, None)?;
                Ok((old, other))
            })
            .unwrap();
        let error = store
            .write(|tx| edit(tx, at(20, 9, 2), &other.id, Some(&old.name), None, None))
            .unwrap_err();
        assert!(error.to_string().contains("add it again"), "{error}");
        let restored = store
            .write(|tx| add(tx, at(20, 9, 3), ids(), "Deen", "#00bb00"))
            .unwrap();
        assert_eq!(restored.id, old.id);
    }
}
