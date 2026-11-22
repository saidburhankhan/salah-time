use super::{stamp, write};
use crate::config::Config;
use chrono::{DateTime, NaiveDate, Utc};
use salah_domain::day::BoundaryId;
use salah_domain::error::Error;
use salah_domain::item::{
    Item, Measure, Repeat, Weekdays, validate_block, validate_repeat, validate_title,
};
use salah_domain::runtime::IdGenerator;
use salah_prayer::schedule;
use salah_store::Store;

const MIN_ID_PREFIX: usize = 4;

/// The next free position at the end of a block.
fn next_position(store: &Store, block: BoundaryId) -> Result<i64, Error> {
    Ok(store
        .items()?
        .iter()
        .filter(|i| i.is_live() && i.block == block)
        .map(|i| i.position + 1)
        .max()
        .unwrap_or(0))
}

/// Finds a live item by its full id or a unique prefix of at least four
/// characters.
pub fn find_item(store: &Store, id: &str) -> Result<Item, Error> {
    let id = id.trim();
    if let Some(item) = store.item(id)?.filter(Item::is_live) {
        return Ok(item);
    }
    if id.len() < MIN_ID_PREFIX {
        return Err(Error::NotFound {
            what: "item",
            id: id.to_owned(),
        });
    }
    let mut matches = store
        .items()?
        .into_iter()
        .filter(|item| item.is_live() && item.id.starts_with(id));
    match (matches.next(), matches.next()) {
        (Some(item), None) => Ok(item),
        (None, _) => Err(Error::NotFound {
            what: "item",
            id: id.to_owned(),
        }),
        (Some(_), Some(_)) => Err(Error::Ambiguous { id: id.to_owned() }),
    }
}

#[derive(Clone, Debug, Default)]
pub struct NewItem {
    pub title: String,
    /// Defaults to the block being planned now.
    pub block: Option<BoundaryId>,
    pub repeat: Option<Repeat>,
    pub weekdays: Option<Weekdays>,
    pub measure: Option<Measure>,
    pub target: Option<i64>,
    /// Day to plan on (one-time) or start from (repeating). Defaults to the
    /// day being planned now.
    pub date: Option<NaiveDate>,
}

pub fn add(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    request: NewItem,
) -> Result<Item, Error> {
    let now = stamp(now);
    let schedule = schedule::resolve(config, now)?;
    let planning = schedule.planning_block();
    let measure = request.measure.unwrap_or(Measure::Check);
    if let Some(target) = request.target {
        if measure == Measure::Check {
            return Err("a target needs --measure minutes or count".into());
        }
        if target <= 0 {
            return Err("target must be positive".into());
        }
    }
    let repeat = request.repeat.unwrap_or(Repeat::None);
    validate_repeat(repeat, request.weekdays)?;
    let day = request.date.unwrap_or(planning.day);
    // Another day has no "current" block, so default to its first block.
    let default_block = if day == planning.day {
        planning.start
    } else {
        BoundaryId::Fajr
    };
    let block = validate_block(request.block.unwrap_or(default_block))?;
    let title = validate_title(&request.title)?;
    write(store, config, now, |tx| {
        let item = Item {
            id: ids.new_id(),
            title: title.clone(),
            block,
            repeat,
            weekdays: request.weekdays,
            measure,
            target: request.target,
            position: next_position(tx, block)?,
            day,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        tx.put_item(&item)?;
        Ok(item)
    })
}

#[derive(Clone, Debug, Default)]
pub struct ItemChanges {
    pub title: Option<String>,
    pub block: Option<BoundaryId>,
    pub target: Option<Option<i64>>,
    pub repeat: Option<Repeat>,
    pub weekdays: Option<Weekdays>,
    pub measure: Option<Measure>,
}

pub fn edit(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    changes: ItemChanges,
) -> Result<Item, Error> {
    let now = stamp(now);
    write(store, config, now, |tx| {
        let mut item = find_item(tx, id)?;
        if let Some(title) = &changes.title {
            item.title = validate_title(title)?;
        }
        if let Some(block) = changes.block {
            item.block = validate_block(block)?;
        }
        if let Some(repeat) = changes.repeat {
            item.repeat = repeat;
            if repeat != Repeat::Weekly {
                item.weekdays = None;
            }
        }
        if let Some(weekdays) = changes.weekdays {
            item.weekdays = Some(weekdays);
        }
        validate_repeat(item.repeat, item.weekdays)?;
        if let Some(measure) = changes.measure {
            item.measure = measure;
            if measure == Measure::Check {
                item.target = None;
            }
        }
        if let Some(target) = changes.target {
            if target.is_some_and(|t| t <= 0) {
                return Err("target must be positive".into());
            }
            if target.is_some() && item.measure == Measure::Check {
                return Err("a target needs a minutes or count item".into());
            }
            item.target = target;
        }
        item.updated_at = now;
        tx.put_item(&item)?;
        Ok(item)
    })
}

/// Moves an item to `block` at `index` (0-based) and renumbers that block.
pub fn move_item(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
    block: BoundaryId,
    index: usize,
) -> Result<Item, Error> {
    let now = stamp(now);
    let block = validate_block(block)?;
    write(store, config, now, |tx| {
        let mut moved = find_item(tx, id)?;
        let mut siblings: Vec<Item> = tx
            .items()?
            .into_iter()
            .filter(|i| i.is_live() && i.block == block && i.id != moved.id)
            .collect();
        siblings.sort_by(|a, b| {
            a.position
                .cmp(&b.position)
                .then(a.created_at.cmp(&b.created_at))
        });
        let moved_id = moved.id.clone();
        moved.block = block;
        siblings.insert(index.min(siblings.len()), moved);
        for (position, item) in siblings.iter_mut().enumerate() {
            let position = position as i64;
            if item.position != position || item.id == moved_id {
                item.position = position;
                item.updated_at = now;
                tx.put_item(item)?;
            }
        }
        siblings
            .into_iter()
            .find(|i| i.id == moved_id)
            .ok_or_else(|| "moved item vanished".into())
    })
}

pub fn remove(
    store: &mut Store,
    config: &Config,
    now: DateTime<Utc>,
    id: &str,
) -> Result<Item, Error> {
    let now = stamp(now);
    write(store, config, now, |tx| {
        let mut item = find_item(tx, id)?;
        item.deleted_at = Some(now);
        item.updated_at = now;
        tx.put_item(&item)?;
        Ok(item)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::test_support::{at, config, date, ids, titled};
    use crate::planner::{day_plan, log, mark, start_timer, today};

    #[test]
    fn add_defaults_to_the_current_block() {
        let mut store = Store::open_in_memory().unwrap();
        let item = add(
            &mut store,
            &config(),
            at(20, 13, 0),
            ids(),
            titled("Call the bank"),
        )
        .unwrap();
        assert_eq!(item.block, BoundaryId::Dhuhr);
        assert_eq!(item.day, date(20));
        let today = today(&store, &config(), at(20, 13, 5)).unwrap();
        assert_eq!(today.open, 1);
        assert_eq!(today.current[0].title, "Call the bank");
    }

    #[test]
    fn the_night_before_fajr_is_a_plannable_block_of_the_coming_day() {
        let mut store = Store::open_in_memory().unwrap();
        // 23:30 on the 20th is after the default 23:00 sleep: the 21st's night.
        let item = add(
            &mut store,
            &config(),
            at(20, 23, 30),
            ids(),
            titled("Tahajjud"),
        )
        .unwrap();
        assert_eq!((item.day, item.block), (date(21), BoundaryId::Night));
        let view = today(&store, &config(), at(21, 2, 0)).unwrap();
        assert_eq!((view.day, view.block), (date(21), Some(BoundaryId::Night)));
        assert_eq!(view.current[0].title, "Tahajjud");
        assert_eq!(view.open, 1);
        let timer = NewItem {
            measure: Some(Measure::Minutes),
            ..titled("Study")
        };
        let study = add(&mut store, &config(), at(21, 3, 0), ids(), timer).unwrap();
        start_timer(&mut store, &config(), at(21, 3, 0), ids(), &study.id).unwrap();
        let plan = day_plan(&store, &config(), at(21, 3, 30), None).unwrap();
        assert_eq!(plan.blocks[0].start, BoundaryId::Night);
        assert_eq!(plan.blocks[0].items.len(), 2);
        assert_eq!(plan.now.as_ref().unwrap().block, Some(BoundaryId::Night));
        assert!(mark(&mut store, &config(), at(21, 3, 0), BoundaryId::Night, None).is_err());
    }

    #[test]
    fn item_added_after_midnight_before_late_bedtime_belongs_to_the_previous_day() {
        let late = Config {
            sleep_time: "00:30".into(),
            ..config()
        };
        let mut store = Store::open_in_memory().unwrap();
        let item = add(
            &mut store,
            &late,
            at(21, 0, 10),
            ids(),
            titled("Review the day"),
        )
        .unwrap();
        assert_eq!((item.day, item.block), (date(20), BoundaryId::Isha));
    }

    #[test]
    fn invalid_requests_change_nothing() {
        let mut store = Store::open_in_memory().unwrap();
        let tick = add(&mut store, &config(), at(20, 13, 0), ids(), titled("Tick")).unwrap();
        assert!(add(&mut store, &config(), at(20, 13, 0), ids(), titled("  ")).is_err());
        let sleep = NewItem {
            block: Some(BoundaryId::Sleep),
            ..titled("Nap")
        };
        assert!(add(&mut store, &config(), at(20, 13, 0), ids(), sleep).is_err());
        let target = NewItem {
            target: Some(5),
            ..titled("Target")
        };
        assert!(add(&mut store, &config(), at(20, 13, 0), ids(), target).is_err());
        assert!(
            log(
                &mut store,
                &config(),
                at(20, 13, 0),
                ids(),
                &tick.id,
                0,
                None
            )
            .is_err()
        );
        assert!(
            log(
                &mut store,
                &config(),
                at(20, 13, 0),
                ids(),
                &tick.id,
                1441,
                None
            )
            .is_err()
        );
        assert!(find_item(&store, "abc").is_err());
        assert_eq!(store.items().unwrap().len(), 1);
        assert!(store.entries().unwrap().is_empty());
    }

    #[test]
    fn ids_match_by_unique_prefix_only() {
        let mut store = Store::open_in_memory().unwrap();
        let mut item = add(&mut store, &config(), at(20, 13, 0), ids(), titled("One")).unwrap();
        item.id = "abcd1111".into();
        store.write(|tx| tx.put_item(&item)).unwrap();
        item.id = "abcd2222".into();
        store.write(|tx| tx.put_item(&item)).unwrap();
        assert!(find_item(&store, "abcd").unwrap_err().contains("ambiguous"));
        assert_eq!(find_item(&store, "abcd1").unwrap().id, "abcd1111");
        assert!(find_item(&store, "abc").is_err(), "too short");
    }

    #[test]
    fn moving_reorders_within_and_between_blocks() {
        let mut store = Store::open_in_memory().unwrap();
        let in_block = |title: &str, block| NewItem {
            block: Some(block),
            ..titled(title)
        };
        let a = add(
            &mut store,
            &config(),
            at(20, 13, 0),
            ids(),
            in_block("A", BoundaryId::Asr),
        )
        .unwrap();
        let b = add(
            &mut store,
            &config(),
            at(20, 13, 0),
            ids(),
            in_block("B", BoundaryId::Asr),
        )
        .unwrap();
        let c = add(
            &mut store,
            &config(),
            at(20, 13, 0),
            ids(),
            in_block("C", BoundaryId::Isha),
        )
        .unwrap();
        assert_eq!((a.position, b.position), (0, 1));
        move_item(
            &mut store,
            &config(),
            at(20, 13, 1),
            &b.id,
            BoundaryId::Asr,
            0,
        )
        .unwrap();
        move_item(
            &mut store,
            &config(),
            at(20, 13, 2),
            &c.id,
            BoundaryId::Asr,
            1,
        )
        .unwrap();
        let plan = day_plan(&store, &config(), at(20, 13, 3), None).unwrap();
        let asr: Vec<_> = plan.blocks[3]
            .items
            .iter()
            .map(|i| i.title.as_str())
            .collect();
        assert_eq!(asr, ["B", "C", "A"]);
        assert!(plan.blocks[5].items.is_empty());
    }

    #[test]
    fn removed_items_disappear_but_keep_a_tombstone() {
        let mut store = Store::open_in_memory().unwrap();
        let item = add(&mut store, &config(), at(20, 13, 0), ids(), titled("Gone")).unwrap();
        remove(&mut store, &config(), at(20, 13, 1), &item.id).unwrap();
        assert!(
            today(&store, &config(), at(20, 13, 2))
                .unwrap()
                .items
                .is_empty()
        );
        assert!(store.item(&item.id).unwrap().unwrap().deleted_at.is_some());
        assert!(find_item(&store, &item.id).is_err());
    }
}
