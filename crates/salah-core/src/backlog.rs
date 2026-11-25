//! The backlog: one-time items without a day or block. They are in no day's
//! plan, capacity or fulfilment until `schedule` gives them a day and block;
//! `to_backlog` takes a one-time item back out of the plan.

use crate::config::Config;
use crate::guard::ensure_version;
use crate::items::{self, ItemView, next_position, one_time};
use crate::stats::NONE;
use crate::{Store, WriteTx, day, projects, tags};
use chrono::{DateTime, NaiveDate, Utc};
use salah_domain::{
    day::BoundaryId,
    error::Error,
    item::validate_block,
    resource::ResourceCounts,
    tracking::{Item, Project},
};
use serde::Serialize;

/// The project of a backlog row, with its default tags.
#[derive(Clone, Debug, Serialize)]
pub struct BacklogProject {
    pub id: String,
    pub name: String,
    pub color: String,
    pub tag_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BacklogRow {
    #[serde(flatten)]
    pub view: ItemView,
    pub project: Option<BacklogProject>,
    pub resources: ResourceCounts,
}

/// Live backlog items in position order. `project_id` and `tag_id` narrow
/// the rows; [`NONE`] selects items without a project (or without a tag).
pub fn list(
    store: &Store,
    today: NaiveDate,
    project_id: Option<&str>,
    tag_id: Option<&str>,
) -> Result<Vec<BacklogRow>, Error> {
    store.read_snapshot(|store| {
        if let Some(id) = project_id.filter(|id| *id != NONE) {
            projects::find(store, id)?;
        }
        if let Some(id) = tag_id.filter(|id| *id != NONE) {
            tags::find(store, id)?;
        }
        let projects = store.projects()?;
        let live_tags: Vec<String> = tags::list(store)?.into_iter().map(|t| t.id).collect();
        let mut counts = store.item_resource_counts()?;
        let mut rows = Vec::new();
        for item in items::list(store)?.into_iter().filter(Item::is_backlog) {
            let project = item
                .project_id
                .as_deref()
                .and_then(|id| projects.iter().find(|p| p.id == id));
            if project.is_some_and(|p| p.archived) {
                continue;
            }
            let tag_ids: Vec<String> = project
                .map(|p| p.tag_ids.clone())
                .unwrap_or_default()
                .into_iter()
                .filter(|id| live_tags.contains(id))
                .collect();
            let keeps_project = match project_id {
                None => true,
                Some(NONE) => item.project_id.is_none(),
                Some(id) => item.project_id.as_deref() == Some(id),
            };
            if !keeps_project {
                continue;
            }
            let view = ItemView::new(store, item, today)?;
            let keeps_tag = match tag_id {
                None => true,
                Some(NONE) => view.tag_ids.is_empty(),
                Some(id) => view.tag_ids.iter().any(|t| t == id),
            };
            if !keeps_tag {
                continue;
            }
            rows.push(BacklogRow {
                resources: counts.remove(&view.item.id).unwrap_or_default(),
                project: project.map(|p| backlog_project(p, tag_ids)),
                view,
            });
        }
        Ok(rows)
    })
}

fn backlog_project(project: &Project, tag_ids: Vec<String>) -> BacklogProject {
    BacklogProject {
        id: project.id.clone(),
        name: project.name.clone(),
        color: project.color.clone(),
        tag_ids,
    }
}

/// Gives a one-time item (in the backlog or already planned) a day from today
/// on and a block, at the end of that block.
pub fn schedule(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    date: NaiveDate,
    block: BoundaryId,
    expected_version: Option<i64>,
) -> Result<Item, Error> {
    let mut item = items::find(tx, id)?;
    ensure_version("item", id, item.version, expected_version)?;
    one_time(item.repeat)?;
    let block = validate_block(block)?;
    let today = day::current(config, now)?;
    if date < today {
        return Err(Error::validation(
            "day",
            "a task cannot be scheduled before today; past days are not re-planned",
        ));
    }
    if tx.day_written(date)? {
        return Err(Error::validation(
            "day",
            "the destination day is already frozen",
        ));
    }
    day::freeze_past(tx, config, today)?;
    tx.preserve_item(&item, today)?;
    if item.day != Some(date) || item.block != Some(block) {
        item.position = next_position(tx, Some(block))?;
    }
    item.block = Some(block);
    item.day = Some(date);
    item.updated_at = now;
    item.version = tx.put_item(&item)?;
    Ok(item)
}

/// Takes a one-time item out of every day from today on and appends it to the
/// backlog. A past day that already lists it keeps it. Already in the backlog:
/// nothing changes.
pub fn to_backlog(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    expected_version: Option<i64>,
) -> Result<Item, Error> {
    let mut item = items::find(tx, id)?;
    ensure_version("item", id, item.version, expected_version)?;
    one_time(item.repeat)?;
    if item.is_backlog() {
        return Ok(item);
    }
    let today = day::current(config, now)?;
    day::freeze_past(tx, config, today)?;
    tx.preserve_item(&item, today)?;
    item.position = next_position(tx, None)?;
    item.block = None;
    item.day = None;
    item.updated_at = now;
    item.version = tx.put_item(&item)?;
    Ok(item)
}
