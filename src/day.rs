use crate::calculation::PrayerTime;
use crate::error::Error;
use serde::{Deserialize, Serialize};

/// A boundary that starts a block of the day. Prayers and the sleep boundary
/// are the only boundaries; Sunrise is shown but never starts a block.
///
/// The serialized names are stored in the database and exports, so they must
/// never change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BoundaryId {
    /// The night block: from the previous evening's sleep time to Fajr. It
    /// starts at the sleep boundary but belongs to the coming day, so it has
    /// its own id. Declared first so it sorts before Fajr.
    Night,
    Fajr,
    Dhuhr,
    Asr,
    Maghrib,
    Isha,
    Sleep,
}

impl BoundaryId {
    /// The five prayers, in daily order.
    pub const PRAYERS: [BoundaryId; 5] = [
        BoundaryId::Fajr,
        BoundaryId::Dhuhr,
        BoundaryId::Asr,
        BoundaryId::Maghrib,
        BoundaryId::Isha,
    ];

    /// The six blocks that can hold items, in daily order: the night before
    /// Fajr, then one block per prayer.
    pub const AWAKE: [BoundaryId; 6] = [
        BoundaryId::Night,
        BoundaryId::Fajr,
        BoundaryId::Dhuhr,
        BoundaryId::Asr,
        BoundaryId::Maghrib,
        BoundaryId::Isha,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            BoundaryId::Night => "night",
            BoundaryId::Fajr => "fajr",
            BoundaryId::Dhuhr => "dhuhr",
            BoundaryId::Asr => "asr",
            BoundaryId::Maghrib => "maghrib",
            BoundaryId::Isha => "isha",
            BoundaryId::Sleep => "sleep",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BoundaryId::Night => "Before Fajr",
            BoundaryId::Fajr => "Fajr",
            BoundaryId::Dhuhr => "Dhuhr",
            BoundaryId::Asr => "Asr",
            BoundaryId::Maghrib => "Maghrib",
            BoundaryId::Isha => "Isha",
            BoundaryId::Sleep => "Sleep",
        }
    }

    pub fn kind(self) -> &'static str {
        match self {
            BoundaryId::Sleep => "sleep",
            BoundaryId::Night => "night",
            _ => "prayer",
        }
    }

    pub fn is_prayer(self) -> bool {
        Self::PRAYERS.contains(&self)
    }

    pub fn parse(value: &str) -> Option<BoundaryId> {
        [Self::AWAKE.as_slice(), &[BoundaryId::Sleep]]
            .concat()
            .into_iter()
            .find(|id| id.as_str() == value)
    }

    /// The boundary that ends the block this boundary starts.
    pub fn next(self) -> Option<BoundaryId> {
        match self {
            BoundaryId::Night => Some(BoundaryId::Fajr),
            BoundaryId::Fajr => Some(BoundaryId::Dhuhr),
            BoundaryId::Dhuhr => Some(BoundaryId::Asr),
            BoundaryId::Asr => Some(BoundaryId::Maghrib),
            BoundaryId::Maghrib => Some(BoundaryId::Isha),
            BoundaryId::Isha => Some(BoundaryId::Sleep),
            BoundaryId::Sleep => None,
        }
    }

    fn of_prayer(prayer: &PrayerTime) -> Option<BoundaryId> {
        BoundaryId::parse(prayer.id).filter(|id| id.is_prayer())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Boundary {
    pub id: BoundaryId,
    /// Local wall-clock minutes after midnight.
    pub minutes: i64,
    /// Calendar day relative to the display date on which the boundary occurs.
    pub day_offset: i64,
    /// Day (Fajr→Sleep) relative to the display date that the boundary
    /// belongs to. A post-midnight bedtime occurs on the next calendar day but
    /// belongs to the previous day.
    pub owner_offset: i64,
}

impl Boundary {
    pub fn name(&self) -> &'static str {
        self.id.name()
    }
    pub fn kind(&self) -> &'static str {
        self.id.kind()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    pub next: Boundary,
    pub current_start: Boundary,
    pub remaining_seconds: i64,
}

impl Timeline {
    pub fn sleeping(&self) -> bool {
        self.current_start.id == BoundaryId::Sleep
    }
}

pub fn parse_clock_minutes(value: &str) -> Result<i64, Error> {
    let bytes = value.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || !bytes[0].is_ascii_digit()
        || !bytes[1].is_ascii_digit()
        || !bytes[3].is_ascii_digit()
        || !bytes[4].is_ascii_digit()
    {
        return Err("time must use HH:MM".into());
    }
    let hours = i64::from(bytes[0] - b'0') * 10 + i64::from(bytes[1] - b'0');
    let minutes = i64::from(bytes[3] - b'0') * 10 + i64::from(bytes[4] - b'0');
    if hours > 23 || minutes > 59 {
        return Err("time is outside the 24-hour clock".into());
    }
    Ok(hours * 60 + minutes)
}

fn prayer_boundary(prayer: &PrayerTime, day_offset: i64) -> Option<Boundary> {
    BoundaryId::of_prayer(prayer).map(|id| Boundary {
        id,
        minutes: prayer.minutes,
        day_offset,
        owner_offset: day_offset,
    })
}

fn sleep_boundary(minutes: i64, owner_offset: i64, sleep_offset: i64) -> Boundary {
    Boundary {
        id: BoundaryId::Sleep,
        minutes,
        day_offset: owner_offset + sleep_offset,
        owner_offset,
    }
}

fn absolute_seconds(boundary: &Boundary) -> i64 {
    boundary.day_offset * 86_400 + boundary.minutes * 60
}

/// Day offset of the sleep boundary that ends the evening of the display date.
///
/// A bedtime earlier than Fajr (for example `00:30`) belongs to the previous
/// evening, so it falls on the next calendar day.
pub fn evening_sleep_offset(sleep_minutes: i64, fajr_minutes: i64) -> i64 {
    i64::from(sleep_minutes < fajr_minutes)
}

pub fn timeline(
    today: &[PrayerTime],
    tomorrow_fajr: &PrayerTime,
    sleep_minutes: i64,
    now_seconds: i64,
) -> Result<Timeline, Error> {
    let fajr_minutes = today
        .iter()
        .find(|prayer| prayer.id == "fajr")
        .ok_or("missing fajr")?
        .minutes;
    let sleep_offset = evening_sleep_offset(sleep_minutes, fajr_minutes);
    let mut candidates: Vec<Boundary> = today
        .iter()
        .filter_map(|prayer| prayer_boundary(prayer, 0))
        .collect();
    // Yesterday's Isha starts the block that a post-midnight bedtime ends.
    // Today's Isha is a close approximation of yesterday's.
    if let Some(isha) = today.iter().find(|prayer| prayer.id == "isha") {
        candidates.extend(prayer_boundary(isha, -1));
    }
    candidates.push(sleep_boundary(sleep_minutes, -1, sleep_offset));
    candidates.push(sleep_boundary(sleep_minutes, 0, sleep_offset));
    candidates.extend(prayer_boundary(tomorrow_fajr, 1));

    let next = candidates
        .iter()
        .filter(|boundary| absolute_seconds(boundary) > now_seconds)
        .min_by_key(|boundary| absolute_seconds(boundary))
        .copied()
        .ok_or("missing next boundary")?;
    let current_start = candidates
        .into_iter()
        .filter(|boundary| absolute_seconds(boundary) <= now_seconds)
        .max_by_key(absolute_seconds)
        .ok_or("missing current block start")?;
    let remaining_seconds = absolute_seconds(&next) - now_seconds;

    Ok(Timeline {
        next,
        current_start,
        remaining_seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prayers() -> Vec<PrayerTime> {
        vec![
            PrayerTime {
                id: "fajr",
                name: "Fajr",
                minutes: 6 * 60,
            },
            PrayerTime {
                id: "sunrise",
                name: "Sunrise",
                minutes: 7 * 60 + 30,
            },
            PrayerTime {
                id: "dhuhr",
                name: "Dhuhr",
                minutes: 12 * 60,
            },
            PrayerTime {
                id: "asr",
                name: "Asr",
                minutes: 15 * 60,
            },
            PrayerTime {
                id: "maghrib",
                name: "Maghrib",
                minutes: 18 * 60,
            },
            PrayerTime {
                id: "isha",
                name: "Isha",
                minutes: 19 * 60 + 30,
            },
        ]
    }

    #[test]
    fn sleep_ends_the_post_isha_block() {
        let tomorrow_fajr = PrayerTime {
            id: "fajr",
            name: "Fajr",
            minutes: 6 * 60,
        };
        let result = timeline(&prayers(), &tomorrow_fajr, 23 * 60, 21 * 3600).unwrap();
        assert_eq!(result.current_start.id, BoundaryId::Isha);
        assert_eq!(result.next.id, BoundaryId::Sleep);
        assert_eq!(result.remaining_seconds, 2 * 3600);
    }

    #[test]
    fn after_sleep_is_the_sleep_to_fajr_block() {
        let tomorrow_fajr = PrayerTime {
            id: "fajr",
            name: "Fajr",
            minutes: 6 * 60,
        };
        let result = timeline(&prayers(), &tomorrow_fajr, 23 * 60, 23 * 3600 + 30 * 60).unwrap();
        assert_eq!(result.current_start.id, BoundaryId::Sleep);
        assert_eq!(result.next.id, BoundaryId::Fajr);
        assert_eq!(result.next.day_offset, 1);
    }

    #[test]
    fn sleep_block_begins_exactly_at_the_configured_time() {
        let tomorrow_fajr = PrayerTime {
            id: "fajr",
            name: "Fajr",
            minutes: 6 * 60,
        };
        let result = timeline(&prayers(), &tomorrow_fajr, 23 * 60, 23 * 3600).unwrap();
        assert_eq!(result.current_start.id, BoundaryId::Sleep);
        assert_eq!(result.next.id, BoundaryId::Fajr);
        assert_eq!(result.remaining_seconds, 7 * 3600);
    }

    #[test]
    fn before_fajr_uses_yesterdays_sleep_as_the_block_start() {
        let tomorrow_fajr = PrayerTime {
            id: "fajr",
            name: "Fajr",
            minutes: 6 * 60,
        };
        let result = timeline(&prayers(), &tomorrow_fajr, 23 * 60, 2 * 3600).unwrap();
        assert_eq!(result.current_start.id, BoundaryId::Sleep);
        assert_eq!(result.current_start.day_offset, -1);
        assert_eq!(result.next.id, BoundaryId::Fajr);
        assert_eq!(result.next.day_offset, 0);
    }

    fn fajr() -> PrayerTime {
        PrayerTime {
            id: "fajr",
            name: "Fajr",
            minutes: 6 * 60,
        }
    }

    #[test]
    fn after_midnight_bedtime_is_next_after_isha() {
        let result = timeline(&prayers(), &fajr(), 30, 21 * 3600).unwrap();
        assert_eq!(result.current_start.id, BoundaryId::Isha);
        assert_eq!(result.next.id, BoundaryId::Sleep);
        assert_eq!(result.next.day_offset, 1);
        assert_eq!(result.remaining_seconds, 3 * 3600 + 30 * 60);
    }

    #[test]
    fn after_midnight_but_before_bedtime_is_still_awake() {
        let result = timeline(&prayers(), &fajr(), 30, 10 * 60).unwrap();
        assert_eq!(result.current_start.id, BoundaryId::Isha);
        assert_eq!(result.current_start.day_offset, -1);
        assert_eq!(result.next.id, BoundaryId::Sleep);
        assert_eq!(result.remaining_seconds, 20 * 60);
    }

    #[test]
    fn after_post_midnight_bedtime_is_sleeping_until_fajr() {
        let result = timeline(&prayers(), &fajr(), 30, 3600).unwrap();
        assert_eq!(result.current_start.id, BoundaryId::Sleep);
        assert_eq!(result.next.id, BoundaryId::Fajr);
        assert_eq!(result.next.day_offset, 0);
    }

    #[test]
    fn post_midnight_sleep_belongs_to_the_previous_day() {
        let result = timeline(&prayers(), &fajr(), 30, 3600).unwrap();
        assert_eq!(result.current_start.day_offset, 0);
        assert_eq!(result.current_start.owner_offset, -1);
        let evening = timeline(&prayers(), &fajr(), 30, 21 * 3600).unwrap();
        assert_eq!(evening.next.day_offset, 1);
        assert_eq!(evening.next.owner_offset, 0);
    }

    #[test]
    fn boundary_ids_round_trip_through_storage_names() {
        for id in BoundaryId::AWAKE.into_iter().chain([BoundaryId::Sleep]) {
            assert_eq!(BoundaryId::parse(id.as_str()), Some(id));
            assert_eq!(
                serde_json::to_string(&id).unwrap(),
                format!("\"{}\"", id.as_str())
            );
        }
        assert_eq!(BoundaryId::parse("sunrise"), None);
    }

    #[test]
    fn clock_parser_is_strict() {
        assert_eq!(parse_clock_minutes("23:05"), Ok(23 * 60 + 5));
        for invalid in ["7:00", "24:00", "23:60", "ab:cd", " 23:00"] {
            assert!(parse_clock_minutes(invalid).is_err(), "{invalid}");
        }
    }
}
