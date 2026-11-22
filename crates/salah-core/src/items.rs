//! Items: work planned into a prayer block, once or on a repeat rule.

use crate::config::Config;
use crate::guard::{ensure_version, minutes, not_found, nullable};
use crate::{Store, WriteTx, projects};
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
    pub planned_minutes: Option<i64>,
    /// The day of a one-time item or the first day of a repeating one.
    /// Defaults to the day in progress.
    pub date: Option<NaiveDate>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ItemChanges {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub block: Option<BoundaryId>,
    #[serde(default)]
    pub repeat: Option<Repeat>,
    #[serde(default)]
    pub weekdays: Option<Weekdays>,
    /// `null` detaches the project; an absent field keeps it.
    #[serde(default, deserialize_with = "nullable")]
    pub project_id: Option<Option<String>>,
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
    pub schedule_text: String,
    /// The first day on or after `from` on which the item applies.
    pub next_occurrence: Option<NaiveDate>,
}

impl ItemView {
    pub fn new(item: Item, from: NaiveDate) -> Self {
        Self {
            schedule_text: item.schedule_text(),
            next_occurrence: item.next_occurrence(from),
            item,
        }
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

/// Live items in block (day order) and position order.
pub fn list(store: &Store) -> Result<Vec<Item>, Error> {
    let mut items: Vec<Item> = store.items()?.into_iter().filter(Item::is_live).collect();
    items.sort_by_key(|item| (item.block, item.position));
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
    let id = id.trim();
    if store.item(id)?.is_some_and(|i| i.is_live()) {
        return Ok(id.to_owned());
    }
    if id.len() < MIN_ID_PREFIX {
        return Err(not_found("item", id));
    }
    let mut matches = list(store)?.into_iter().filter(|i| i.id.starts_with(id));
    match (matches.next(), matches.next()) {
        (Some(item), None) => Ok(item.id),
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
    let day = new.date.unwrap_or(planning.day);
    let default_block = if day == planning.day {
        planning.start
    } else {
        BoundaryId::Fajr
    };
    let block = validate_block(new.block.unwrap_or(default_block))?;
    let repeat = new.repeat.unwrap_or(Repeat::None);
    validate_repeat(repeat, new.weekdays)?;
    let mut item = Item {
        id: ids.new_id(),
        title: validate_title(&new.title)?,
        project_id: checked_project(tx, new.project_id)?,
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
    Ok(item)
}

pub fn edit(
    tx: &WriteTx<'_>,
    now: DateTime<Utc>,
    id: &str,
    changes: ItemChanges,
    expected_version: Option<i64>,
) -> Result<Item, Error> {
    let mut item = find(tx, id)?;
    ensure_version("item", id, item.version, expected_version)?;
    if let Some(title) = &changes.title {
        item.title = validate_title(title)?;
    }
    if let Some(block) = changes.block
        && block != item.block
    {
        item.block = validate_block(block)?;
        item.position = next_position(tx, block)?;
    }
    if let Some(repeat) = changes.repeat {
        item.repeat = repeat;
        if repeat != Repeat::Weekly {
            item.weekdays = None;
        }
    }
    if changes.weekdays.is_some() {
        item.weekdays = changes.weekdays;
    }
    validate_repeat(item.repeat, item.weekdays)?;
    if let Some(project_id) = changes.project_id {
        item.project_id = checked_project(tx, project_id)?;
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
    now: DateTime<Utc>,
    id: &str,
    block: BoundaryId,
    position: MovePosition,
    expected_version: Option<i64>,
) -> Result<Vec<Item>, Error> {
    let block = validate_block(block)?;
    let mut moved = find(tx, id)?;
    ensure_version("item", id, moved.version, expected_version)?;
    let mut siblings: Vec<Item> = list(tx)?
        .into_iter()
        .filter(|i| i.block == block && i.id != moved.id)
        .collect();
    let index = match &position {
        MovePosition::Index(index) => (*index).min(siblings.len()),
        MovePosition::After(anchor) => sibling_index(&siblings, anchor)? + 1,
        MovePosition::Before(anchor) => sibling_index(&siblings, anchor)?,
    };
    moved.block = block;
    let moved_id = moved.id.clone();
    siblings.insert(index, moved);
    let mut changed = Vec::new();
    for (position, mut item) in siblings.into_iter().enumerate() {
        let position = position as i64;
        if item.position != position || item.id == moved_id {
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
    now: DateTime<Utc>,
    id: &str,
    expected_version: Option<i64>,
) -> Result<Item, Error> {
    let mut item = find(tx, id)?;
    ensure_version("item", id, item.version, expected_version)?;
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

fn sibling_index(siblings: &[Item], anchor: &str) -> Result<usize, Error> {
    siblings
        .iter()
        .position(|i| i.id == anchor)
        .ok_or_else(|| not_found("item", anchor))
}

fn next_position(store: &Store, block: BoundaryId) -> Result<i64, Error> {
    Ok(list(store)?
        .iter()
        .filter(|i| i.block == block)
        .map(|i| i.position + 1)
        .max()
        .unwrap_or(0))
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
        assert_eq!((first.block, first.day), (BoundaryId::Dhuhr, date(20)));
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
        assert_eq!(other_day.block, BoundaryId::Fajr);
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
            .write(|tx| edit(tx, at(20, 14, 0), &item.id, changes.clone(), Some(1)))
            .unwrap();
        assert_eq!(edited.project_id.as_deref(), Some(project.id.as_str()));
        assert_eq!((edited.planned_minutes, edited.version), (None, 2));
        let stale = store.write(|tx| edit(tx, at(20, 14, 0), &item.id, changes.clone(), Some(1)));
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
                    at(20, 13, 5),
                    &c.id,
                    BoundaryId::Dhuhr,
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
}
