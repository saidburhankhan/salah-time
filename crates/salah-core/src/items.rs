//! Items: work planned into a prayer block, once or on a repeat rule. A
//! one-time item may instead wait in the backlog (see [`crate::backlog`]).

use crate::config::Config;
use crate::guard::{ensure_version, minutes, not_found, nullable};
use crate::sessions::{self, PromptMetric};
use crate::{Store, WriteTx, day, metrics, projects};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use salah_domain::{
    day::BoundaryId,
    error::Error,
    item::{
        Repeat, Weekdays, next_occurrence, schedule_text, validate_block, validate_repeat,
        validate_title,
    },
    runtime::IdGenerator,
    tracking::Item,
};
use salah_prayer::schedule;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The shortest id prefix the human CLI resolves.
const MIN_ID_PREFIX: usize = 4;

#[derive(Clone, Debug, Default)]
pub struct NewItem {
    pub title: String,
    /// Defaults to the block in progress (or Fajr on another day).
    pub block: Option<BoundaryId>,
    pub repeat: Option<Repeat>,
    pub weekdays: Option<Weekdays>,
    pub project_id: Option<String>,
    pub epic_id: Option<String>,
    pub metric_ids: Option<Vec<String>>,
    pub tag_ids: Option<Vec<String>>,
    pub planned_minutes: Option<i64>,
    /// The day of a one-time item or the first day of a repeating one.
    /// Defaults to the day in progress.
    pub date: Option<NaiveDate>,
    /// A one-time item with no day or block yet.
    pub backlog: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ItemChanges {
    #[serde(default)]
    pub title: Option<String>,
    pub tag_ids: Option<Vec<String>>,
    #[serde(default)]
    pub block: Option<BoundaryId>,
    #[serde(default)]
    pub repeat: Option<Repeat>,
    #[serde(default)]
    pub weekdays: Option<Weekdays>,
    /// `null` detaches the project; an absent field keeps it.
    #[serde(default, deserialize_with = "nullable")]
    pub project_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "nullable")]
    pub epic_id: Option<Option<String>>,
    /// `null` inherits, an empty list asks for nothing, an absent field keeps the selection.
    #[serde(default, deserialize_with = "nullable")]
    pub metric_ids: Option<Option<Vec<String>>>,
    /// `null` clears the plan; an absent field keeps it.
    #[serde(default, deserialize_with = "nullable")]
    pub planned_minutes: Option<Option<i64>>,
}

/// Where a moved item lands among the items of its target block.
#[derive(Clone, Debug)]
pub enum MovePosition {
    Index(usize),
    After(String),
    Before(String),
}

/// A stored item with its display recurrence.
#[derive(Clone, Debug, Serialize)]
pub struct ItemView {
    #[serde(flatten)]
    pub item: Item,
    pub tag_ids: Vec<String>,
    /// No day and no block: the item waits in the backlog.
    pub backlog: bool,
    pub metrics: Vec<PromptMetric>,
    pub schedule_text: String,
    /// The first day on or after `from` on which the item applies.
    pub next_occurrence: Option<NaiveDate>,
}

impl ItemView {
    pub fn new(store: &Store, item: Item, from: NaiveDate) -> Result<Self, Error> {
        Ok(Self {
            tag_ids: store.item_tag_ids(&item.id)?,
            metrics: effective_metrics(store, &item)?,
            schedule_text: item.schedule_text(),
            next_occurrence: item.next_occurrence(from),
            backlog: item.is_backlog(),
            item,
        })
    }
}

/// The recurrence preview of an item draft. Reads nothing.
#[derive(Clone, Debug, Serialize)]
pub struct Preview {
    pub next_occurrence: NaiveDate,
    /// Occurrences from the draft date to the end of its month.
    pub occurrences_this_month: usize,
    pub schedule_text: String,
}

/// Live items in block (day order) and position order, the backlog last.
pub fn list(store: &Store) -> Result<Vec<Item>, Error> {
    let mut items: Vec<Item> = store.items()?.into_iter().filter(Item::is_live).collect();
    items.sort_by_key(|item| (item.is_backlog(), item.block, item.position));
    Ok(items)
}

pub fn find(store: &Store, id: &str) -> Result<Item, Error> {
    store
        .item(id)?
        .filter(Item::is_live)
        .ok_or_else(|| not_found("item", id))
}

/// Resolves a full id or a unique prefix of at least four characters. A
/// convenience of the human CLI; RPC callers always send full ids.
pub fn resolve_id(store: &Store, id: &str) -> Result<String, Error> {
    resolve_item_id(store, id, false)
}

/// Report filters can select historical work on a soft-deleted task.
pub fn resolve_report_id(store: &Store, id: &str) -> Result<String, Error> {
    resolve_item_id(store, id, true)
}

fn resolve_item_id(store: &Store, id: &str, include_removed: bool) -> Result<String, Error> {
    let id = id.trim();
    if store
        .item(id)?
        .is_some_and(|i| include_removed || i.is_live())
    {
        return Ok(id.to_owned());
    }
    if id.len() < MIN_ID_PREFIX {
        return Err(not_found("item", id));
    }
    let mut matches = store.item_ids_with_prefix(id, include_removed)?.into_iter();
    match (matches.next(), matches.next()) {
        (Some(id), None) => Ok(id),
        (None, _) => Err(not_found("item", id)),
        (Some(_), Some(_)) => Err(Error::Ambiguous { id: id.to_owned() }),
    }
}

pub fn add(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    new: NewItem,
) -> Result<Item, Error> {
    let planning = schedule::resolve(config, now)?.planning_block();
    day::freeze_past(tx, config, planning.day)?;
    let repeat = new.repeat.unwrap_or(Repeat::None);
    validate_repeat(repeat, new.weekdays)?;
    let (block, day) = if new.backlog {
        if new.block.is_some() || new.date.is_some() {
            return Err(Error::validation(
                "backlog",
                "a backlog task has no day or block; schedule it later",
            ));
        }
        one_time(repeat)?;
        (None, None)
    } else {
        let day = new.date.unwrap_or(planning.day);
        if day < planning.day {
            return Err(Error::validation(
                "date",
                "an item cannot start before today; past days are not re-planned",
            ));
        }
        let default_block = if day == planning.day {
            planning.start
        } else {
            BoundaryId::Fajr
        };
        let block = validate_block(new.block.unwrap_or(default_block))?;
        (Some(block), Some(day))
    };
    let (project_id, epic_id) = epic_owner(tx, new.project_id, new.epic_id)?;
    let project_id = checked_project(tx, project_id)?;
    let metric_ids = checked_metric_ids(tx, project_id.as_deref(), new.metric_ids)?;
    let tags = match new.tag_ids {
        Some(ids) => projects::checked_tags(tx, ids)?,
        None => project_id
            .as_deref()
            .map(|id| tx.project_default_tags(id))
            .transpose()?
            .unwrap_or_default(),
    };
    let mut item = Item {
        id: ids.new_id(),
        title: validate_title(&new.title)?,
        project_id,
        epic_id,
        metric_ids,
        block,
        repeat,
        weekdays: new.weekdays,
        day,
        planned_minutes: new
            .planned_minutes
            .map(|m| minutes("planned_minutes", m))
            .transpose()?,
        position: next_position(tx, block)?,
        created_at: now,
        updated_at: now,
        deleted_at: None,
        version: 1,
    };
    item.version = tx.put_item(&item)?;
    tx.set_item_tags(&item.id, &tags)?;
    Ok(item)
}

pub fn edit(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    changes: ItemChanges,
    expected_version: Option<i64>,
) -> Result<Item, Error> {
    let mut item = find(tx, id)?;
    ensure_version("item", id, item.version, expected_version)?;
    let today = day::current(config, now)?;
    day::freeze_past(tx, config, today)?;
    tx.preserve_item(&item, today)?;
    if let Some(title) = &changes.title {
        item.title = validate_title(title)?;
    }
    if let Some(block) = changes.block
        && Some(block) != item.block
    {
        if item.is_backlog() {
            return Err(backlog_has_no_block());
        }
        item.block = Some(validate_block(block)?);
        item.position = next_position(tx, item.block)?;
    }
    if let Some(repeat) = changes.repeat
        && repeat != item.repeat
    {
        if item.is_backlog() {
            one_time(repeat)?;
        }
        item.repeat = repeat;
        if repeat != Repeat::Weekly {
            item.weekdays = None;
        }
    }
    if changes.weekdays.is_some() {
        item.weekdays = changes.weekdays;
    }
    validate_repeat(item.repeat, item.weekdays)?;
    let previous_project = item.project_id.clone();
    let project_given = changes.project_id.is_some();
    if let Some(project_id) = changes.project_id {
        item.project_id = checked_project(tx, project_id)?;
    }
    // A new project resets the epic and the metric selection unless the same
    // edit chooses them, which are then checked against the new project.
    match changes.epic_id {
        Some(Some(epic_id)) if item.epic_id.as_ref() != Some(&epic_id) => {
            let epic = crate::epics::assignable(tx, &epic_id)?;
            // An item without a project joins the epic's project.
            if item.project_id.is_none() && !project_given {
                item.project_id = Some(projects::active(tx, &epic.project_id)?.id);
            }
            item.epic_id = Some(epic_id);
        }
        Some(epic_id) => item.epic_id = epic_id,
        None if item.project_id != previous_project => item.epic_id = None,
        None => {}
    }
    match changes.metric_ids {
        Some(ids) => item.metric_ids = checked_metric_ids(tx, item.project_id.as_deref(), ids)?,
        None if item.project_id != previous_project => item.metric_ids = None,
        None => {}
    }
    if let Some(tags) = changes.tag_ids {
        tx.set_item_tags(id, &projects::checked_tags(tx, tags)?)?;
    }
    if let Some(planned) = changes.planned_minutes {
        item.planned_minutes = planned.map(|m| minutes("planned_minutes", m)).transpose()?;
    }
    item.updated_at = now;
    item.version = tx.put_item(&item)?;
    Ok(item)
}

/// Moves an item into `block` at `position` and renumbers that block.
/// Returns every item whose block or position changed, the moved one first.
pub fn move_to(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    block: Option<BoundaryId>,
    position: MovePosition,
    expected_version: Option<i64>,
) -> Result<Vec<Item>, Error> {
    let block = block.map(validate_block).transpose()?;
    let mut moved = find(tx, id)?;
    ensure_version("item", id, moved.version, expected_version)?;
    // A move stays inside the backlog or among the scheduled blocks;
    // `item.schedule` and `item.backlog` cross between them.
    match (moved.is_backlog(), block.is_none()) {
        (true, false) => return Err(backlog_has_no_block()),
        (false, true) => {
            return Err(Error::validation(
                "block",
                "only a backlog task moves with block null; use item.backlog",
            ));
        }
        _ => {}
    }
    let today = day::current(config, now)?;
    day::freeze_past(tx, config, today)?;
    let mut siblings: Vec<Item> = list(tx)?
        .into_iter()
        .filter(|i| i.block == block && i.id != moved.id)
        .collect();
    let index = match &position {
        MovePosition::Index(index) => (*index).min(siblings.len()),
        MovePosition::After(anchor) => sibling_index(&siblings, anchor)? + 1,
        MovePosition::Before(anchor) => sibling_index(&siblings, anchor)?,
    };
    tx.preserve_item(&moved, today)?;
    moved.block = block;
    let moved_id = moved.id.clone();
    siblings.insert(index, moved);
    let mut changed = Vec::new();
    for (position, mut item) in siblings.into_iter().enumerate() {
        let position = position as i64;
        if item.position != position || item.id == moved_id {
            if item.id != moved_id {
                tx.preserve_item(&item, today)?;
            }
            item.position = position;
            item.updated_at = now;
            item.version = tx.put_item(&item)?;
            changed.push(item);
        }
    }
    changed.sort_by_key(|i| i.id != moved_id);
    Ok(changed)
}

pub fn remove(
    tx: &WriteTx<'_>,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    expected_version: Option<i64>,
) -> Result<Item, Error> {
    let mut item = find(tx, id)?;
    ensure_version("item", id, item.version, expected_version)?;
    let today = day::current(config, now)?;
    day::freeze_past(tx, config, today)?;
    tx.preserve_item(&item, today)?;
    crate::goals::end_scope(
        tx,
        salah_domain::goal::Scope {
            item_id: Some(id.into()),
            ..Default::default()
        },
        today,
        now,
    )?;
    item.deleted_at = Some(now);
    item.updated_at = now;
    item.version = tx.put_item(&item)?;
    Ok(item)
}

/// Validates a draft like `add` and describes its recurrence from `date`.
pub fn preview(
    block: BoundaryId,
    repeat: Repeat,
    weekdays: Option<Weekdays>,
    planned_minutes: Option<i64>,
    date: NaiveDate,
) -> Result<Preview, Error> {
    validate_block(block)?;
    validate_repeat(repeat, weekdays)?;
    if let Some(planned) = planned_minutes {
        minutes("planned_minutes", planned)?;
    }
    let next = next_occurrence(repeat, weekdays, date)
        .ok_or_else(|| Error::validation("weekdays", "a weekly item needs at least one weekday"))?;
    let occurrences_this_month = if repeat == Repeat::None {
        1
    } else {
        date.iter_days()
            .take_while(|day| day.month() == date.month())
            .filter(|day| next_occurrence(repeat, weekdays, *day) == Some(*day))
            .count()
    };
    Ok(Preview {
        next_occurrence: next,
        occurrences_this_month,
        schedule_text: schedule_text(repeat, weekdays),
    })
}

/// Effective prompt for an item, shared by item views and the running timer.
pub(crate) fn effective_metrics(store: &Store, item: &Item) -> Result<Vec<PromptMetric>, Error> {
    sessions::selected_prompt(
        store,
        item.project_id.as_deref(),
        item.metric_ids.as_deref(),
    )
}

fn checked_metric_ids(
    store: &Store,
    project_id: Option<&str>,
    selection: Option<Vec<String>>,
) -> Result<Option<Vec<String>>, Error> {
    let Some(ids) = selection else {
        return Ok(None);
    };
    let asked = project_id
        .map(|id| store.project_metrics(id))
        .transpose()?
        .unwrap_or_default();
    let mut seen = BTreeSet::new();
    for id in &ids {
        let metric = metrics::find(store, id).map_err(|error| match error {
            Error::NotFound { .. } => {
                Error::validation("metric_ids", format!("unknown metric {id:?}"))
            }
            error => error,
        })?;
        if metric.archived || (!metric.builtin && !asked.iter().any(|link| link.metric_id == *id)) {
            return Err(Error::validation(
                "metric_ids",
                format!("metric {id:?} is not available for this item"),
            ));
        }
        if !seen.insert(id) {
            return Err(Error::validation(
                "metric_ids",
                format!("metric {id:?} is listed twice"),
            ));
        }
    }
    Ok(Some(ids))
}

fn sibling_index(siblings: &[Item], anchor: &str) -> Result<usize, Error> {
    siblings
        .iter()
        .position(|i| i.id == anchor)
        .ok_or_else(|| not_found("item", anchor))
}

/// The position after every live item in `block` (`None`: the backlog).
pub(crate) fn next_position(store: &Store, block: Option<BoundaryId>) -> Result<i64, Error> {
    Ok(list(store)?
        .iter()
        .filter(|i| i.block == block)
        .map(|i| i.position + 1)
        .max()
        .unwrap_or(0))
}

/// Only one-time tasks can wait in the backlog.
pub(crate) fn one_time(repeat: Repeat) -> Result<(), Error> {
    if repeat != Repeat::None {
        return Err(Error::validation(
            "repeat",
            "only one-time tasks can be in the backlog",
        ));
    }
    Ok(())
}

fn backlog_has_no_block() -> Error {
    Error::validation(
        "block",
        "a backlog task has no block; schedule it with item.schedule",
    )
}

/// A chosen epic must be active; without a project the item takes the epic's.
fn epic_owner(
    store: &Store,
    project_id: Option<String>,
    epic_id: Option<String>,
) -> Result<(Option<String>, Option<String>), Error> {
    let Some(epic_id) = epic_id else {
        return Ok((project_id, None));
    };
    let epic = crate::epics::assignable(store, &epic_id)?;
    Ok((project_id.or(Some(epic.project_id)), Some(epic_id)))
}

fn checked_project(store: &Store, id: Option<String>) -> Result<Option<String>, Error> {
    id.map(|id| projects::active(store, &id).map(|p| p.id))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{at, config, date, ids, project};

    fn titled(title: &str) -> NewItem {
        NewItem {
            title: title.into(),
            ..NewItem::default()
        }
    }

    #[test]
    fn add_defaults_to_the_block_in_progress_and_appends() {
        let mut store = Store::open_in_memory().unwrap();
        let (first, second) = store
            .write(|tx| {
                Ok((
                    add(tx, &config(), at(20, 13, 0), ids(), titled("Email"))?,
                    add(tx, &config(), at(20, 13, 0), ids(), titled("Call"))?,
                ))
            })
            .unwrap();
        assert_eq!(
            (first.block, first.day),
            (Some(BoundaryId::Dhuhr), Some(date(20)))
        );
        assert_eq!((first.position, second.position), (0, 1));
        let other_day = store
            .write(|tx| {
                add(
                    tx,
                    &config(),
                    at(20, 13, 0),
                    ids(),
                    NewItem {
                        date: Some(date(22)),
                        ..titled("Later")
                    },
                )
            })
            .unwrap();
        assert_eq!(other_day.block, Some(BoundaryId::Fajr));
    }

    #[test]
    fn edit_attaches_a_project_and_null_clears_the_plan() {
        let mut store = Store::open_in_memory().unwrap();
        let project = project(&mut store, "Quran");
        let item = store
            .write(|tx| {
                add(
                    tx,
                    &config(),
                    at(20, 13, 0),
                    ids(),
                    NewItem {
                        planned_minutes: Some(30),
                        ..titled("Read")
                    },
                )
            })
            .unwrap();
        let changes: ItemChanges = serde_json::from_value(
            serde_json::json!({"project_id": project.id, "planned_minutes": null}),
        )
        .unwrap();
        let edited = store
            .write(|tx| {
                edit(
                    tx,
                    &config(),
                    at(20, 14, 0),
                    &item.id,
                    changes.clone(),
                    Some(1),
                )
            })
            .unwrap();
        assert_eq!(edited.project_id.as_deref(), Some(project.id.as_str()));
        assert_eq!((edited.planned_minutes, edited.version), (None, 2));
        let stale = store.write(|tx| {
            edit(
                tx,
                &config(),
                at(20, 14, 0),
                &item.id,
                changes.clone(),
                Some(1),
            )
        });
        assert_eq!(stale.unwrap_err().code(), "conflict");
    }

    #[test]
    fn move_renumbers_the_target_block_around_an_anchor() {
        let mut store = Store::open_in_memory().unwrap();
        let [a, b, c] = ["A", "B", "C"].map(|title| {
            store
                .write(|tx| add(tx, &config(), at(20, 13, 0), ids(), titled(title)))
                .unwrap()
        });
        let changed = store
            .write(|tx| {
                move_to(
                    tx,
                    &config(),
                    at(20, 13, 5),
                    &c.id,
                    Some(BoundaryId::Dhuhr),
                    MovePosition::After(a.id.clone()),
                    None,
                )
            })
            .unwrap();
        assert_eq!(changed[0].id, c.id);
        let order: Vec<_> = list(&store).unwrap().into_iter().map(|i| i.title).collect();
        assert_eq!(order, ["A", "C", "B"]);
        assert!(store.item(&b.id).unwrap().unwrap().position == 2);
    }

    #[test]
    fn prefixes_resolve_only_when_unique_and_long_enough() {
        let mut store = Store::open_in_memory().unwrap();
        let item = store
            .write(|tx| add(tx, &config(), at(20, 13, 0), ids(), titled("Email")))
            .unwrap();
        assert_eq!(resolve_id(&store, &item.id[..8]).unwrap(), item.id);
        assert_eq!(resolve_id(&store, "abc").unwrap_err().code(), "not_found");
    }

    #[test]
    fn preview_counts_the_rest_of_the_month() {
        let mondays = Some(Weekdays::parse_list("mon").unwrap());
        let preview = preview(
            BoundaryId::Fajr,
            Repeat::Weekly,
            mondays,
            Some(20),
            date(20),
        )
        .unwrap();
        assert_eq!(preview.next_occurrence, date(23));
        assert_eq!(preview.occurrences_this_month, 2, "23 and 30 November");
        assert_eq!(preview.schedule_text, "Mon");
        assert!(super::preview(BoundaryId::Sleep, Repeat::None, None, None, date(20)).is_err());
        assert!(super::preview(BoundaryId::Fajr, Repeat::None, None, Some(0), date(20)).is_err());
    }

    #[test]
    fn an_item_cannot_start_before_today() {
        let mut store = Store::open_in_memory().unwrap();
        let back_dated = store.write(|tx| {
            add(
                tx,
                &config(),
                at(20, 13, 0),
                ids(),
                NewItem {
                    date: Some(date(19)),
                    ..titled("Yesterday")
                },
            )
        });
        assert_eq!(back_dated.unwrap_err().code(), "validation");
    }

    #[test]
    fn a_project_change_keeps_metric_ids_sent_with_it_checked_against_the_new_project() {
        use crate::metrics::NewMetric;
        use crate::projects::{MetricChoice, NewProject};
        use salah_domain::tracking::MetricKind;
        let mut store = Store::open_in_memory().unwrap();
        let first = project(&mut store, "Quran");
        let (pages, second, item) = store
            .write(|tx| {
                let pages = metrics::add(
                    tx,
                    at(20, 9, 0),
                    ids(),
                    NewMetric {
                        name: "Pages".into(),
                        unit: "pages".into(),
                        kind: MetricKind::Count,
                        scale_max: None,
                        rollup: None,
                    },
                )?;
                let second = projects::add(
                    tx,
                    at(20, 9, 0),
                    ids(),
                    NewProject {
                        name: "Books".into(),
                        metrics: Some(vec![MetricChoice {
                            metric_id: pages.id.clone(),
                            required: false,
                        }]),
                        ..NewProject::default()
                    },
                )?;
                let item = add(
                    tx,
                    &config(),
                    at(20, 13, 0),
                    ids(),
                    NewItem {
                        project_id: Some(first.id.clone()),
                        metric_ids: Some(vec!["focus".into()]),
                        ..titled("Read")
                    },
                )?;
                Ok((pages, second, item))
            })
            .unwrap();
        let edit = |store: &mut Store, body: serde_json::Value| {
            let changes: ItemChanges = serde_json::from_value(body).unwrap();
            store.write(|tx| {
                edit(
                    tx,
                    &config(),
                    at(20, 14, 0),
                    &item.id,
                    changes.clone(),
                    None,
                )
            })
        };
        let moved = edit(
            &mut store,
            serde_json::json!({"project_id": second.id, "metric_ids": [pages.id, "focus"]}),
        )
        .unwrap();
        assert_eq!(moved.project_id.as_deref(), Some(second.id.as_str()));
        assert_eq!(
            moved.metric_ids,
            Some(vec![pages.id.clone(), "focus".into()])
        );
        let refused = edit(
            &mut store,
            serde_json::json!({"project_id": first.id, "metric_ids": [pages.id]}),
        );
        assert!(matches!(
            refused,
            Err(Error::Validation {
                field: "metric_ids",
                ..
            })
        ));
        let reset = edit(&mut store, serde_json::json!({"project_id": first.id})).unwrap();
        assert_eq!(
            reset.metric_ids, None,
            "without metric_ids a new project resets"
        );
    }
}
