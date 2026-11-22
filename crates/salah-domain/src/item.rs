//! Plan rules shared by items and their previews: recurrence, weekdays,
//! title and block validation, and prayer marks. Free of storage and clocks.

use crate::day::BoundaryId;
use crate::error::Error;
use crate::tracking::default_version;
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
    pub fn parse_list(text: &str) -> Result<Weekdays, Error> {
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

pub fn validate_title(title: &str) -> Result<String, Error> {
    let title = title.trim();
    if title.is_empty() {
        return Err("title must not be empty".into());
    }
    if title.chars().count() > TITLE_MAX_CHARS {
        return Err(format!("title must be at most {TITLE_MAX_CHARS} characters").into());
    }
    if title.chars().any(char::is_control) {
        return Err("title must not contain control characters".into());
    }
    Ok(title.to_string())
}

/// A weekly item needs weekdays; any other item must not have them.
pub fn validate_repeat(repeat: Repeat, weekdays: Option<Weekdays>) -> Result<(), Error> {
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
    #[serde(default = "default_version")]
    pub version: i64,
}

impl PrayerMark {
    pub fn id_for(day: NaiveDate, prayer: BoundaryId) -> String {
        format!("{}:{}", day.format("%Y-%m-%d"), prayer.as_str())
    }

    pub fn is_live(&self) -> bool {
        self.deleted_at.is_none()
    }
}

pub fn validate_block(block: BoundaryId) -> Result<BoundaryId, Error> {
    if block == BoundaryId::Sleep {
        return Err("items cannot be planned in the sleeping block".into());
    }
    Ok(block)
}

/// First occurrence of a repeat rule on or after `from`.
pub fn next_occurrence(
    repeat: Repeat,
    weekdays: Option<Weekdays>,
    from: NaiveDate,
) -> Option<NaiveDate> {
    match repeat {
        Repeat::None => Some(from),
        Repeat::Daily => Some(from),
        Repeat::Weekly => {
            let days = weekdays.filter(|days| !days.is_empty())?;
            let mut day = from;
            for _ in 0..=7 {
                if days.contains(day.weekday()) {
                    return Some(day);
                }
                day = day.succ_opt()?;
            }
            None
        }
    }
}

/// Locale-neutral English schedule label shared by day rows and previews.
pub fn schedule_text(repeat: Repeat, weekdays: Option<Weekdays>) -> String {
    match repeat {
        Repeat::None => "Once".into(),
        Repeat::Daily => "Daily".into(),
        Repeat::Weekly => {
            let Some(days) = weekdays else {
                return "Weekly".into();
            };
            let names = days.names();
            if names.is_empty() {
                return "Weekly".into();
            }
            if names.len() == 7 {
                return "Daily".into();
            }
            if names == ["mon", "tue", "wed", "thu", "fri"] {
                return "Mon–Fri".into();
            }
            let short = |name: &str| {
                let mut chars = name.chars();
                chars
                    .next()
                    .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                    .unwrap_or_default()
            };
            names
                .iter()
                .map(|name| short(name))
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
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
    fn schedule_text_is_locale_neutral() {
        let custom = Weekdays::parse_list("mon,wed,fri").unwrap();
        assert_eq!(schedule_text(Repeat::Daily, None), "Daily");
        assert_eq!(schedule_text(Repeat::None, None), "Once");
        assert_eq!(
            schedule_text(Repeat::Weekly, Some(Weekdays::WORKWEEK)),
            "Mon–Fri"
        );
        assert_eq!(schedule_text(Repeat::Weekly, Some(custom)), "Mon, Wed, Fri");
    }

    #[test]
    fn next_occurrence_crosses_month_and_year_boundaries() {
        let monday = Some(Weekdays::parse_list("mon").unwrap());
        // 2026-12-31 is a Thursday; the next Monday is 2027-01-04.
        assert_eq!(
            next_occurrence(Repeat::Weekly, monday, d(2026, 12, 31)),
            Some(d(2027, 1, 4))
        );
        assert_eq!(
            next_occurrence(Repeat::Weekly, monday, d(2026, 11, 27)),
            Some(d(2026, 11, 30))
        );
        assert_eq!(next_occurrence(Repeat::Weekly, None, d(2026, 11, 27)), None);
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
