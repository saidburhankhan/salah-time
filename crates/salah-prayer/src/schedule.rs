//! Resolves the prayer schedule and the day's blocks for a moment in time.
//!
//! Status, items, and the desktop app all ask this module which block is
//! current and when each block starts and ends, so schedule rules live in one
//! place.

use crate::calculation::{calculate, method};
use crate::config::Config;
use crate::official;
use chrono::{DateTime, Duration, FixedOffset, NaiveDate, TimeZone, Timelike, Utc};
pub use salah_domain::day::BlockKey;
use salah_domain::day::{self, Boundary, BoundaryId, PrayerTime, Timeline};
use salah_domain::error::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub id: &'static str,
    pub name: &'static str,
    pub updated: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockWindow {
    pub key: BlockKey,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct Schedule {
    /// The local civil date the prayer rows describe.
    pub date: NaiveDate,
    pub now_seconds: i64,
    pub method_name: &'static str,
    pub source: Source,
    pub prayers: Vec<PrayerTime>,
    pub timeline: Timeline,
}

impl Schedule {
    /// Calendar date on which a boundary occurs.
    pub fn occurs_on(&self, boundary: &Boundary) -> NaiveDate {
        shift(self.date, boundary.day_offset)
    }

    /// The day a boundary belongs to.
    pub fn owner_day(&self, boundary: &Boundary) -> NaiveDate {
        shift(self.date, boundary.owner_offset)
    }

    /// The day that is in progress. Sleep ends a day, so between sleep and
    /// Fajr this is the coming day, in its night block.
    pub fn day(&self) -> NaiveDate {
        let owner = self.owner_day(&self.timeline.current_start);
        if self.timeline.sleeping() {
            shift(owner, 1)
        } else {
            owner
        }
    }

    /// The block now in progress; `night` between sleep and Fajr.
    pub fn current_block(&self) -> BlockKey {
        let start = if self.timeline.sleeping() {
            BoundaryId::Night
        } else {
            self.timeline.current_start.id
        };
        BlockKey {
            day: self.day(),
            start,
        }
    }

    /// Where something planned "now" belongs: the block in progress.
    pub fn planning_block(&self) -> BlockKey {
        self.current_block()
    }
}

fn shift(date: NaiveDate, days: i64) -> NaiveDate {
    date.checked_add_signed(Duration::days(days))
        .unwrap_or(date)
}

fn offset(config: &Config) -> Result<FixedOffset, Error> {
    FixedOffset::east_opt((config.timezone_offset * 3600.0).round() as i32)
        .ok_or_else(|| "invalid timezone offset".into())
}

/// The UTC instant at which `boundary` occurs relative to `date` (the local
/// civil date the boundary offsets are based on).
pub fn boundary_at(
    config: &Config,
    date: NaiveDate,
    boundary: &Boundary,
) -> Result<DateTime<Utc>, Error> {
    let zone = offset(config)?;
    let midnight = shift(date, boundary.day_offset)
        .and_hms_opt(0, 0, 0)
        .ok_or("invalid date")?;
    let local = zone
        .from_local_datetime(&midnight)
        .single()
        .ok_or("invalid local time")?;
    Ok((local + Duration::minutes(boundary.minutes)).with_timezone(&Utc))
}

fn calculated(config: &Config, date: NaiveDate) -> Result<Vec<PrayerTime>, Error> {
    let selected =
        method(&config.method).ok_or_else(|| format!("unknown method: {}", config.method))?;
    Ok(calculate(
        date,
        config.latitude,
        config.longitude,
        config.timezone_offset,
        selected,
        config.asr == "hanafi",
    ))
}

/// Prayer times for a civil date: the cached official timetable when it
/// covers that date, otherwise the offline calculation.
pub fn prayers_for(config: &Config, date: NaiveDate) -> Result<(Vec<PrayerTime>, Source), Error> {
    let selected =
        method(&config.method).ok_or_else(|| format!("unknown method: {}", config.method))?;
    let official_schedule = if config.method == "uzbekistan" {
        official::load(date, &config.location)
    } else {
        None
    };
    Ok(match official_schedule {
        Some(schedule) => (
            official::prayer_times(&schedule),
            Source {
                id: "official",
                name: "Muslim Board of Uzbekistan",
                updated: Some(schedule.fetched_at.to_rfc3339()),
            },
        ),
        None => (
            calculated(config, date)?,
            Source {
                id: "calculated",
                name: selected.name,
                updated: None,
            },
        ),
    })
}

pub fn resolve(config: &Config, utc_now: DateTime<Utc>) -> Result<Schedule, Error> {
    let selected =
        method(&config.method).ok_or_else(|| format!("unknown method: {}", config.method))?;
    let now = utc_now.with_timezone(&offset(config)?);
    let date = now.date_naive();
    let (prayers, source) = prayers_for(config, date)?;
    let now_seconds = i64::from(now.num_seconds_from_midnight());
    let tomorrow = date.succ_opt().ok_or("date overflow")?;
    let tomorrow_fajr = calculated(config, tomorrow)?
        .into_iter()
        .find(|p| p.id == "fajr")
        .ok_or("missing fajr")?;
    let sleep_minutes = day::parse_clock_minutes(&config.sleep_time)?;
    let timeline = day::timeline(&prayers, &tomorrow_fajr, sleep_minutes, now_seconds)?;
    Ok(Schedule {
        date,
        now_seconds,
        method_name: selected.name,
        source,
        prayers,
        timeline,
    })
}

/// The six blocks of a day with their UTC start and end instants. The night
/// block runs from the previous evening's sleep time to Fajr; the Isha block
/// ends at this evening's sleep time.
pub fn day_blocks(config: &Config, day: NaiveDate) -> Result<Vec<BlockWindow>, Error> {
    Ok(day_schedule(config, day)?.blocks)
}

/// The block windows and the timetable they were derived from.
pub struct DaySchedule {
    pub blocks: Vec<BlockWindow>,
    pub prayers: Vec<PrayerTime>,
    pub source: Source,
    pub sleep_at: DateTime<Utc>,
    pub offset_seconds: i32,
}

pub fn day_schedule(config: &Config, day: NaiveDate) -> Result<DaySchedule, Error> {
    let zone = offset(config)?;
    let (prayers, source) = prayers_for(config, day)?;
    let sleep_minutes = day::parse_clock_minutes(&config.sleep_time)?;
    let minutes_of = |id: BoundaryId| -> Result<i64, Error> {
        Ok(prayers
            .iter()
            .find(|p| p.id == id.as_str())
            .map(|p| p.minutes)
            .ok_or_else(|| format!("missing {}", id.as_str()))?)
    };
    let fajr = minutes_of(BoundaryId::Fajr)?;
    let instant = |days: i64, minutes: i64| -> Result<DateTime<Utc>, Error> {
        let midnight = shift(day, days)
            .and_hms_opt(0, 0, 0)
            .ok_or("invalid date")?;
        let local = zone
            .from_local_datetime(&midnight)
            .single()
            .ok_or("invalid local time")?;
        Ok((local + Duration::minutes(minutes)).with_timezone(&Utc))
    };
    let sleep_offset = day::evening_sleep_offset(sleep_minutes, fajr);
    let sleep = instant(sleep_offset, sleep_minutes)?;
    // Yesterday's Fajr is close enough to today's to place last night's sleep.
    let last_night = instant(sleep_offset - 1, sleep_minutes)?;
    let blocks = BoundaryId::AWAKE
        .into_iter()
        .map(|id| {
            let start = match id {
                BoundaryId::Night => last_night,
                _ => instant(0, minutes_of(id)?)?,
            };
            let end = match id.next() {
                Some(BoundaryId::Sleep) | None => sleep,
                Some(next) => instant(0, minutes_of(next)?)?,
            };
            Ok(BlockWindow {
                key: BlockKey { day, start: id },
                start,
                // A sleep time configured before Isha leaves an empty block
                // rather than one that ends before it starts.
                end: end.min(sleep).max(start),
            })
        })
        .collect::<Result<_, Error>>()?;
    Ok(DaySchedule {
        blocks,
        prayers,
        source,
        sleep_at: sleep,
        offset_seconds: zone.local_minus_utc(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tashkent() -> Config {
        // Use a method without an official cache so tests never read the
        // user's cached timetable.
        Config {
            method: "mwl".into(),
            ..Config::default()
        }
    }

    fn at(h: u32, m: u32, date: (i32, u32, u32)) -> DateTime<Utc> {
        // Tashkent is UTC+5.
        FixedOffset::east_opt(5 * 3600)
            .unwrap()
            .with_ymd_and_hms(date.0, date.1, date.2, h, m, 0)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn afternoon_belongs_to_the_same_civil_day() {
        let schedule = resolve(&tashkent(), at(13, 0, (2026, 11, 20))).unwrap();
        assert_eq!(
            schedule.current_block(),
            BlockKey {
                day: date(2026, 11, 20),
                start: BoundaryId::Dhuhr
            }
        );
    }

    #[test]
    fn after_midnight_before_a_late_bedtime_is_still_the_previous_day() {
        let config = Config {
            sleep_time: "00:30".into(),
            ..tashkent()
        };
        let schedule = resolve(&config, at(0, 10, (2026, 11, 21))).unwrap();
        assert_eq!(
            schedule.current_block(),
            BlockKey {
                day: date(2026, 11, 20),
                start: BoundaryId::Isha
            }
        );
    }

    #[test]
    fn after_sleep_the_coming_day_starts_with_its_night_block() {
        for moment in [at(23, 30, (2026, 11, 20)), at(2, 0, (2026, 11, 21))] {
            let schedule = resolve(&tashkent(), moment).unwrap();
            assert!(schedule.timeline.sleeping());
            assert_eq!(schedule.day(), date(2026, 11, 21));
            assert_eq!(
                schedule.current_block(),
                BlockKey {
                    day: date(2026, 11, 21),
                    start: BoundaryId::Night
                }
            );
            assert_eq!(schedule.planning_block(), schedule.current_block());
        }
    }

    #[test]
    fn day_blocks_are_contiguous_and_end_at_sleep() {
        let config = Config {
            sleep_time: "00:30".into(),
            ..tashkent()
        };
        let blocks = day_blocks(&config, date(2026, 11, 20)).unwrap();
        assert_eq!(blocks.len(), 6);
        assert_eq!(blocks[0].key.start, BoundaryId::Night);
        assert_eq!(blocks[0].start, at(0, 30, (2026, 11, 20)));
        for pair in blocks.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
        assert_eq!(blocks[5].key.start, BoundaryId::Isha);
        assert_eq!(blocks[5].end, at(0, 30, (2026, 11, 21)));
    }

    #[test]
    fn block_key_serializes_as_stable_strings() {
        let key = BlockKey {
            day: date(2026, 11, 20),
            start: BoundaryId::Asr,
        };
        assert_eq!(
            serde_json::to_string(&key).unwrap(),
            r#"{"day":"2026-11-20","start":"asr"}"#
        );
    }
}
