//! Items and entries: the planning domain, free of storage and clocks.
//!
//! An item is an intention placed in a block ("Read Quran after Fajr"). An
//! entry is something that happened to an item on a day (ticked, 20 minutes,
//! 3 times, a timer session). Items never record history themselves, so
//! history, totals, and future streaks are all derived from entries.

use crate::day::BoundaryId;
use crate::schedule::BlockKey;
use chrono::{DateTime, Datelike, NaiveDate, Utc, Weekday};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const TITLE_MAX_CHARS: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    /// Planned once, for the item's `day`.
    None,
    /// Planned every day from the day it was created.
    Daily,
    /// Planned on the item's `weekdays` from the day it was created.
    Weekly,
}

const WEEKDAY_NAMES: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/// A set of weekdays, stored as a bitmask (Monday = bit 0) and written as a
/// list of names (`["mon", "fri"]`) in JSON and exports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Weekdays(pub u8);

impl Weekdays {
    pub const WORKWEEK: Weekdays = Weekdays(0b001_1111);
    pub const ALL: u8 = 0b111_1111;

    pub fn contains(self, day: Weekday) -> bool {
        self.0 & (1 << day.num_days_from_monday()) != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 & Self::ALL == 0
    }

    /// Parses a comma-separated list such as `mon,wed,fri`.
    pub fn parse_list(text: &str) -> Result<Weekdays, String> {
        let mut mask = 0u8;
        for name in text.split(',').map(str::trim).filter(|n| !n.is_empty()) {
            mask |= 1 << weekday_index(name).ok_or_else(|| format!("unknown weekday {name:?}"))?;
        }
        if mask == 0 {
            return Err("choose at least one weekday".into());
        }
        Ok(Weekdays(mask))
    }

    pub fn names(self) -> Vec<&'static str> {
        (0..7)
            .filter(|i| self.0 & (1 << i) != 0)
            .map(|i| WEEKDAY_NAMES[i])
            .collect()
    }
}

/// Accepts `mon`, `monday`, `mondays` in any case.
pub fn weekday_index(name: &str) -> Option<u8> {
    let name = name.to_ascii_lowercase();
    let short = name.get(..3)?;
    WEEKDAY_NAMES
        .iter()
        .position(|d| *d == short)
        .filter(|_| {
            let full = [
                "monday",
                "tuesday",
                "wednesday",
                "thursday",
                "friday",
                "saturday",
                "sunday",
            ];
            name.len() == 3 || full.iter().any(|f| name == *f || name == format!("{f}s"))
        })
        .map(|i| i as u8)
}

impl Serialize for Weekdays {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.names().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Weekdays {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let names = Vec::<String>::deserialize(deserializer)?;
        Weekdays::parse_list(&names.join(",")).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Measure {
    Check,
    Minutes,
    Count,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Done,
    Minutes,
    Count,
    Timer,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub title: String,
    /// The block the item is planned in.
    pub block: BoundaryId,
    pub repeat: Repeat,
    /// The days a weekly item applies on; `None` unless `repeat` is weekly.
    #[serde(default)]
    pub weekdays: Option<Weekdays>,
    pub measure: Measure,
    /// Minutes or count that completes the item for a day, if any.
    pub target: Option<i64>,
    /// Order within the block, ascending.
    #[serde(default)]
    pub position: i64,
    /// For one-time items, the day they are planned for. For repeating items,
    /// the first day they apply to.
    pub day: NaiveDate,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub item_id: String,
    /// The block the entry happened in.
    pub block: BlockKey,
    pub kind: EntryKind,
    /// Minutes or count for `minutes`/`count` entries.
    pub value: Option<i64>,
    pub started_at: Option<DateTime<Utc>>,
    /// `None` while a timer is running.
    pub ended_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

pub fn validate_title(title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("title must not be empty".into());
    }
    if title.chars().count() > TITLE_MAX_CHARS {
        return Err(format!(
            "title must be at most {TITLE_MAX_CHARS} characters"
        ));
    }
    if title.chars().any(char::is_control) {
        return Err("title must not contain control characters".into());
    }
    Ok(title.to_string())
}

/// A weekly item needs weekdays; any other item must not have them.
pub fn validate_repeat(repeat: Repeat, weekdays: Option<Weekdays>) -> Result<(), String> {
    match (repeat, weekdays) {
        (Repeat::Weekly, Some(days)) if !days.is_empty() && days.0 & !Weekdays::ALL == 0 => Ok(()),
        (Repeat::Weekly, _) => Err("a weekly item needs at least one weekday".into()),
        (_, Some(_)) => Err("weekdays apply only to weekly items".into()),
        (_, None) => Ok(()),
    }
}

/// A prayer marked as prayed on a day. The id is `<day>:<prayer>`, so two
/// devices marking the same prayer produce the same row and merge cleanly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PrayerMark {
    pub id: String,
    pub day: NaiveDate,
    pub prayer: BoundaryId,
    pub marked_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl PrayerMark {
    pub fn id_for(day: NaiveDate, prayer: BoundaryId) -> String {
        format!("{}:{}", day.format("%Y-%m-%d"), prayer.as_str())
    }

    pub fn is_live(&self) -> bool {
        self.deleted_at.is_none()
    }
}

pub fn validate_block(block: BoundaryId) -> Result<BoundaryId, String> {
    if block == BoundaryId::Sleep {
        return Err("items cannot be planned in the sleeping block".into());
    }
    Ok(block)
}

impl Item {
    pub fn is_live(&self) -> bool {
        self.deleted_at.is_none()
    }

    /// Whether the item is planned on `day`.
    pub fn applies_to(&self, day: NaiveDate) -> bool {
        self.is_live()
            && match self.repeat {
                Repeat::None => self.day == day,
                Repeat::Daily => self.day <= day,
                Repeat::Weekly => {
                    self.day <= day && self.weekdays.is_some_and(|w| w.contains(day.weekday()))
                }
            }
    }
}

impl Entry {
    pub fn is_live(&self) -> bool {
        self.deleted_at.is_none()
    }

    pub fn is_running_timer(&self) -> bool {
        self.is_live() && self.kind == EntryKind::Timer && self.ended_at.is_none()
    }

    /// Whole minutes this entry contributes. A running timer counts up to
    /// `now`, never past `block_end`.
    pub fn minutes(&self, now: DateTime<Utc>, block_end: Option<DateTime<Utc>>) -> i64 {
        if !self.is_live() {
            return 0;
        }
        match self.kind {
            EntryKind::Minutes => self.value.unwrap_or(0),
            EntryKind::Timer => {
                let Some(start) = self.started_at else {
                    return 0;
                };
                let mut end = self.ended_at.unwrap_or(now);
                if let Some(limit) = block_end {
                    end = end.min(limit);
                }
                (end - start).num_minutes().max(0)
            }
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Open,
    Done,
    /// The day ended without the item being done.
    Missed,
}

/// One item as it stands on one day.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DayItem {
    pub id: String,
    pub title: String,
    pub block: BoundaryId,
    pub repeat: Repeat,
    pub weekdays: Option<Weekdays>,
    pub measure: Measure,
    pub target: Option<i64>,
    pub position: i64,
    pub state: State,
    pub minutes: i64,
    pub count: i64,
    pub timer_started_at: Option<DateTime<Utc>>,
}

/// What the day looks like from the point of view of a moment in it.
#[derive(Clone, Copy, Debug)]
pub struct DayView {
    pub day: NaiveDate,
    /// The block in progress if `day` is today; `None` for past and future
    /// days.
    pub current: Option<BoundaryId>,
    /// True when `day` has ended.
    pub ended: bool,
    pub now: DateTime<Utc>,
}

/// Every item planned on a day, with its state derived from the entries.
/// `block_end` gives the end instant of a block, used to cap running timers.
pub fn day_items(
    items: &[Item],
    entries: &[Entry],
    view: DayView,
    mut block_end: impl FnMut(BlockKey) -> Option<DateTime<Utc>>,
) -> Vec<DayItem> {
    let mut rows: Vec<DayItem> = items
        .iter()
        .filter(|item| item.applies_to(view.day))
        .map(|item| {
            let mine = || {
                entries
                    .iter()
                    .filter(|e| e.is_live() && e.item_id == item.id && e.block.day == view.day)
            };
            // Adjustments can subtract, but a day never goes below zero.
            let minutes = mine()
                .map(|e| e.minutes(view.now, block_end(e.block)))
                .sum::<i64>()
                .max(0);
            let count = mine()
                .filter(|e| e.kind == EntryKind::Count)
                .filter_map(|e| e.value)
                .sum::<i64>()
                .max(0);
            let ticked = mine().any(|e| e.kind == EntryKind::Done);
            let reached = match (item.measure, item.target) {
                (Measure::Minutes, Some(target)) => minutes >= target,
                (Measure::Count, Some(target)) => count >= target,
                _ => false,
            };
            let state = if ticked || reached {
                State::Done
            } else if view.ended {
                State::Missed
            } else {
                State::Open
            };
            let timer_started_at = mine()
                .find(|e| e.is_running_timer())
                .filter(|e| block_end(e.block).is_none_or(|end| view.now < end))
                .and_then(|e| e.started_at);
            DayItem {
                id: item.id.clone(),
                title: item.title.clone(),
                block: item.block,
                repeat: item.repeat,
                weekdays: item.weekdays,
                measure: item.measure,
                target: item.target,
                position: item.position,
                state,
                minutes,
                count,
                timer_started_at,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        a.block
            .cmp(&b.block)
            .then_with(|| a.position.cmp(&b.position))
            .then_with(|| a.title.cmp(&b.title))
    });
    rows
}

/// Items shown for the block in progress: those planned in it, plus open
/// items carried forward from earlier blocks of the same day.
pub fn current_block_items(rows: &[DayItem], current: BoundaryId) -> Vec<DayItem> {
    rows.iter()
        .filter(|row| row.block == current || (row.block < current && row.state == State::Open))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 20, h, m, 0).unwrap()
    }
    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
    }
    fn item(id: &str, block: BoundaryId, repeat: Repeat, measure: Measure) -> Item {
        Item {
            id: id.into(),
            title: id.into(),
            block,
            repeat,
            weekdays: None,
            measure,
            target: None,
            position: 0,
            day: d(20),
            created_at: t(0, 0),
            updated_at: t(0, 0),
            deleted_at: None,
        }
    }
    fn entry(item_id: &str, block: BoundaryId, kind: EntryKind) -> Entry {
        Entry {
            id: format!("{item_id}-{kind:?}"),
            item_id: item_id.into(),
            block: BlockKey {
                day: d(20),
                start: block,
            },
            kind,
            value: None,
            started_at: None,
            ended_at: None,
            created_at: t(0, 0),
            updated_at: t(0, 0),
            deleted_at: None,
        }
    }
    fn view(current: Option<BoundaryId>, ended: bool, now: DateTime<Utc>) -> DayView {
        DayView {
            day: d(20),
            current,
            ended,
            now,
        }
    }

    #[test]
    fn open_items_carry_forward_within_the_day() {
        let items = [
            item("early", BoundaryId::Fajr, Repeat::None, Measure::Check),
            item("done", BoundaryId::Fajr, Repeat::None, Measure::Check),
            item("later", BoundaryId::Isha, Repeat::None, Measure::Check),
        ];
        let entries = [entry("done", BoundaryId::Fajr, EntryKind::Done)];
        let rows = day_items(
            &items,
            &entries,
            view(Some(BoundaryId::Asr), false, t(10, 0)),
            |_| None,
        );
        let shown: Vec<_> = current_block_items(&rows, BoundaryId::Asr)
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(shown, ["early"]);
    }

    #[test]
    fn unfinished_items_are_missed_after_the_day_ends() {
        let items = [item("a", BoundaryId::Dhuhr, Repeat::None, Measure::Check)];
        let rows = day_items(&items, &[], view(None, true, t(23, 0)), |_| None);
        assert_eq!(rows[0].state, State::Missed);
    }

    #[test]
    fn daily_items_apply_from_their_first_day_only() {
        let daily = item("q", BoundaryId::Fajr, Repeat::Daily, Measure::Minutes);
        assert!(!daily.applies_to(d(19)));
        assert!(daily.applies_to(d(20)) && daily.applies_to(d(25)));
        let once = item("o", BoundaryId::Fajr, Repeat::None, Measure::Check);
        assert!(!once.applies_to(d(21)));
    }

    #[test]
    fn running_timer_is_capped_at_its_block_end() {
        let mut quran = item("q", BoundaryId::Fajr, Repeat::Daily, Measure::Minutes);
        quran.target = Some(30);
        let mut timer = entry("q", BoundaryId::Fajr, EntryKind::Timer);
        timer.started_at = Some(t(6, 40));
        let rows = day_items(
            &[quran],
            &[timer],
            view(Some(BoundaryId::Dhuhr), false, t(13, 0)),
            |_| Some(t(7, 0)),
        );
        assert_eq!(rows[0].minutes, 20);
        assert_eq!(rows[0].timer_started_at, None, "stopped at block end");
        assert_eq!(rows[0].state, State::Open);
    }

    #[test]
    fn reaching_a_target_completes_the_item() {
        let mut pushups = item("p", BoundaryId::Asr, Repeat::Daily, Measure::Count);
        pushups.target = Some(50);
        let mut first = entry("p", BoundaryId::Asr, EntryKind::Count);
        first.value = Some(30);
        let mut second = first.clone();
        second.id = "p2".into();
        second.value = Some(20);
        let rows = day_items(
            &[pushups],
            &[first, second],
            view(Some(BoundaryId::Asr), false, t(11, 0)),
            |_| None,
        );
        assert_eq!(rows[0].count, 50);
        assert_eq!(rows[0].state, State::Done);
    }

    #[test]
    fn deleted_items_and_entries_are_ignored() {
        let mut gone = item("g", BoundaryId::Fajr, Repeat::None, Measure::Check);
        gone.deleted_at = Some(t(1, 0));
        let keep = item("k", BoundaryId::Fajr, Repeat::None, Measure::Check);
        let mut undone = entry("k", BoundaryId::Fajr, EntryKind::Done);
        undone.deleted_at = Some(t(2, 0));
        let rows = day_items(
            &[gone, keep],
            &[undone],
            view(Some(BoundaryId::Fajr), false, t(3, 0)),
            |_| None,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, State::Open);
    }

    #[test]
    fn weekly_items_apply_on_their_weekdays_only() {
        let mut weekly = item("w", BoundaryId::Asr, Repeat::Weekly, Measure::Check);
        weekly.weekdays = Some(Weekdays::WORKWEEK);
        // 2026-11-20 is a Friday; 21 Saturday; 23 Monday.
        assert!(weekly.applies_to(d(20)));
        assert!(!weekly.applies_to(d(21)));
        assert!(weekly.applies_to(d(23)));
        assert!(!weekly.applies_to(d(13)), "before its first day");
    }

    #[test]
    fn weekdays_parse_and_serialize_as_names() {
        let days = Weekdays::parse_list("mon, Wednesday,fridays").unwrap();
        assert_eq!(days.names(), ["mon", "wed", "fri"]);
        assert_eq!(
            serde_json::to_string(&days).unwrap(),
            r#"["mon","wed","fri"]"#
        );
        let back: Weekdays = serde_json::from_str(r#"["fri","mon","wed"]"#).unwrap();
        assert_eq!(back, days);
        assert!(Weekdays::parse_list("").is_err());
        assert!(Weekdays::parse_list("mon,funday").is_err());
        assert!(validate_repeat(Repeat::Weekly, None).is_err());
        assert!(validate_repeat(Repeat::Daily, Some(days)).is_err());
        assert!(validate_repeat(Repeat::Weekly, Some(days)).is_ok());
    }

    #[test]
    fn titles_are_validated() {
        assert_eq!(
            validate_title("  Call the bank "),
            Ok("Call the bank".into())
        );
        assert!(validate_title("   ").is_err());
        assert!(validate_title("a\nb").is_err());
        assert!(validate_title(&"x".repeat(201)).is_err());
        assert!(validate_block(BoundaryId::Sleep).is_err());
    }
}
