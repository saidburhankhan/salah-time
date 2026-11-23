use crate::config::Config;
use crate::guard::{ensure_version, label, not_found, note, nullable};
use crate::{Store, WriteTx, day, projects};
use chrono::{DateTime, NaiveDate, Utc};
use salah_domain::{
    epic::{Epic, EpicStatus, EpicView},
    error::Error,
    runtime::IdGenerator,
    tracking::Item,
};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct NewEpic {
    pub project_id: String,
    pub name: String,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EpicChanges {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "nullable")]
    pub note: Option<Option<String>>,
    pub status: Option<EpicStatus>,
}

pub fn list(
    store: &Store,
    project: Option<&str>,
    status: Option<EpicStatus>,
    today: NaiveDate,
) -> Result<Vec<EpicView>, Error> {
    store.epic_views(project, status, today)
}

pub fn find(store: &Store, id: &str) -> Result<Epic, Error> {
    store
        .epic(id)?
        .filter(|e| e.deleted_at.is_none())
        .ok_or_else(|| not_found("epic", id))
}

pub fn add(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    new: NewEpic,
) -> Result<Epic, Error> {
    projects::active(tx, &new.project_id)?;
    let mut epic = Epic {
        id: ids.new_id(),
        position: tx.next_epic_position(&new.project_id)?,
        project_id: new.project_id,
        name: label("name", &new.name, 60)?,
        note: note(new.note)?,
        status: EpicStatus::Active,
        finished_at: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
        version: 1,
    };
    epic.version = tx.put_epic(&epic)?;
    Ok(epic)
}

pub fn edit(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    id: &str,
    changes: EpicChanges,
    expected_version: Option<i64>,
) -> Result<Epic, Error> {
    let mut epic = find(tx, id)?;
    ensure_version("epic", id, epic.version, expected_version)?;
    if let Some(name) = changes.name {
        epic.name = label("name", &name, 60)?;
    }
    if let Some(value) = changes.note {
        epic.note = note(value)?;
    }
    if let Some(status) = changes.status
        && status != epic.status
    {
        epic.status = status;
        epic.finished_at = (status == EpicStatus::Finished).then_some(now);
    }
    epic.updated_at = now;
    epic.version = tx.put_epic(&epic)?;
    Ok(epic)
}

pub fn remove(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    expected_version: Option<i64>,
) -> Result<Vec<Item>, Error> {
    let mut epic = find(tx, id)?;
    ensure_version("epic", id, epic.version, expected_version)?;
    if tx.epic_has_sessions(id)? {
        return Err(Error::Conflict(
            "this epic has sessions; finish it instead".into(),
        ));
    }
    let mut items = tx.items_in_epic(id)?;
    if !items.is_empty() {
        let today = day::current(config, now)?;
        day::freeze_past(tx, config, today)?;
        for item in &mut items {
            tx.preserve_item(item, today)?;
            item.epic_id = None;
            item.updated_at = now;
            item.version = tx.put_item(item)?;
        }
    }
    epic.deleted_at = Some(now);
    epic.updated_at = now;
    tx.put_epic(&epic)?;
    Ok(items)
}
